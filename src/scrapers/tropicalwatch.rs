use async_trait::async_trait;
use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::sync::Arc;
use tracing::{error, info};
use url::Url;

use crate::config::{Config, SiteConfig};
use crate::models::{Site, WatchListing};
use crate::parsers::{clean_text, format_price_eur_display, get_price_string_for_hash, 
                      parse_year_from_string,
                      extract_reference, parse_table_th_td};
use crate::scrapers::WatchScraper;
use crate::utils::http::fetch_with_retry;
use crate::utils::exchange_rate::ExchangeRateClient;

pub struct TropicalWatchScraper {
    config: Arc<Config>,
    exchange_rate_client: Arc<ExchangeRateClient>,
}

impl TropicalWatchScraper {
    pub fn new(config: Arc<Config>, exchange_rate_client: Arc<ExchangeRateClient>) -> Self {
        Self { config, exchange_rate_client }
    }
}

#[derive(Clone)]
struct WatchData {
    url: String,
    title: String,
    price_usd_raw: String,
    image_url: String,
}

// Known brands list for TropicalWatch
const KNOWN_BRANDS: &[&str] = &[
    "Rolex", "Patek Philippe", "Audemars Piguet", "Omega", "Tudor", "Heuer", 
    "Studio Underd0g", "Longines", "Jaeger-LeCoultre", "Zenith", "IWC", 
    "Panerai", "Cartier", "Breitling", "Universal Geneve", "A. Lange & Söhne"
];

#[async_trait]
impl WatchScraper for TropicalWatchScraper {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>> {
        let site_config = self.site_config();
        info!("Scraping Tropical Watch...");
        
        // Get USD to EUR exchange rate
        let eur_rate = self.exchange_rate_client.get_usd_to_eur_rate(client).await?;
        
        let response = fetch_with_retry(client, &site_config.url, 3).await?;
        let html = response.text().await?;
        
        // Extract all data synchronously
        let watch_data = extract_watch_data(&html, &site_config.base_url)?;
        
        info!("Found {} watch items on Tropical Watch listing page", watch_data.len());
        
        let mut listings = Vec::new();
        
        // Process each watch with async operations
        for data in watch_data {
            if !data.url.is_empty() {
                match self.process_watch(data, client, site_config, eur_rate).await {
                    Ok(listing) => listings.push(listing),
                    Err(e) => error!("Error parsing Tropical Watch item: {}", e),
                }
            }
        }
        
        Ok(listings)
    }
    
    fn site_config(&self) -> &SiteConfig {
        &self.config.sites["tropicalwatch"]
    }
    
    fn site_key(&self) -> Site {
        Site::TropicalWatch
    }
}

fn extract_watch_data(html: &str, base_url: &str) -> Result<Vec<WatchData>> {
    let document = Html::parse_document(html);
    let watch_selector = Selector::parse("li.watch")
        .map_err(|_| anyhow::anyhow!("Failed to parse watch selector"))?;
    
    let mut watch_data = Vec::new();
    
    for element in document.select(&watch_selector) {
        let mut data = WatchData {
            url: String::new(),
            title: String::new(),
            price_usd_raw: String::new(),
            image_url: String::new(),
        };
        
        // Extract link
        if let Ok(link_selector) = Selector::parse("div.photo-wrapper a") {
            if let Some(link) = element.select(&link_selector).next() {
                if let Some(href) = link.value().attr("href") {
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(full_url) = base.join(href) {
                            data.url = full_url.to_string();
                        }
                    }
                }
            }
        }
        
        // Extract title
        if let Ok(title_selector) = Selector::parse("div.content a h2") {
            if let Some(title_elem) = element.select(&title_selector).next() {
                data.title = clean_text(&title_elem.text().collect::<String>());
            }
        }
        
        // Extract price in USD
        if let Ok(price_selector) = Selector::parse("div.content a h3") {
            if let Some(price_elem) = element.select(&price_selector).next() {
                let price_text = clean_text(&price_elem.text().collect::<String>());
                data.price_usd_raw = get_price_string_for_hash(&price_text);
            }
        }
        
        // Extract image
        if let Ok(img_selector) = Selector::parse("div.photo-wrapper a img") {
            if let Some(img) = element.select(&img_selector).next() {
                if let Some(src) = img.value().attr("src") {
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(full_url) = base.join(src) {
                            data.image_url = full_url.to_string();
                        }
                    }
                }
            }
        }
        
        watch_data.push(data);
    }
    
    Ok(watch_data)
}

impl TropicalWatchScraper {
    async fn process_watch(
        &self,
        data: WatchData,
        client: &Client,
        site_config: &SiteConfig,
        eur_rate: f64,
    ) -> Result<WatchListing> {
        let mut watch = WatchListing {
            site_name: site_config.name.clone(),
            watch_url: data.url.clone(),
            image_url: data.image_url,
            title: data.title.clone(),
            price_usd_raw_for_hash: Some(data.price_usd_raw.clone()),
            ..Default::default()
        };
        
        // Convert USD to EUR
        if !data.price_usd_raw.is_empty() && data.price_usd_raw != "❓" {
            match data.price_usd_raw.parse::<f64>() {
                Ok(usd_price) => {
                    let eur_price = usd_price * eur_rate;
                    watch.price_eur_display = format_price_eur_display(&eur_price.to_string());
                }
                Err(_) => {
                    // Try extracting numeric value
                    let re = Regex::new(r"[\d,.]+")?;
                    if let Some(m) = re.find(&data.price_usd_raw) {
                        let price_str = m.as_str().replace(",", "");
                        if let Ok(usd_price) = price_str.parse::<f64>() {
                            let eur_price = usd_price * eur_rate;
                            watch.price_eur_display = format_price_eur_display(&eur_price.to_string());
                        }
                    }
                }
            }
        }
        
        // Fetch detail page for additional information
        info!("Fetching details for Tropical Watch item (URL: {})", data.url);
        
        // Add delay to be respectful
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        match fetch_with_retry(client, &data.url, 3).await {
            Ok(detail_response) => {
                let detail_html = detail_response.text().await?;
                parse_detail_page(&detail_html, &mut watch);
            }
            Err(e) => {
                error!("Could not fetch detail page for {}: {}", data.url, e);
            }
        }
        
        Ok(watch)
    }
}

fn parse_detail_page(html: &str, watch: &mut WatchListing) {
    let document = Html::parse_document(html);
    
    // Get detailed title
    if let Ok(title_selector) = Selector::parse("h1.watch-main-title") {
        if let Some(title_elem) = document.select(&title_selector).next() {
            watch.title = clean_text(&title_elem.text().collect::<String>());
        }
    }
    
    // Parse details table
    if let Ok(table_selector) = Selector::parse("div.watch-main-details-content table.watch-main-details-table") {
        if let Some(table) = document.select(&table_selector).next() {
            let table_html = table.html();
            let headers_map = std::collections::HashMap::from([
                ("Year", "year_text"),
                ("Brand", "brand_table"),
                ("Model", "model_table"),
                ("Reference", "reference_text"),
                ("Case Material", "case_material_table"),
                ("Diameter", "diameter_table"),
            ]);
            
            let details = parse_table_th_td(&table_html, &headers_map);
            
            // Extract brand
            if let Some(brand) = details.get("brand_table") {
                watch.brand = clean_text(brand);
            }
            
            // Extract model
            if let Some(model) = details.get("model_table") {
                watch.model = clean_text(model);
            }
            
            // Extract year
            if let Some(year_text) = details.get("year_text") {
                watch.year = parse_year_from_string(year_text, Some(&watch.title));
            }
            
            // Extract reference
            if let Some(ref_text) = details.get("reference_text") {
                watch.reference = extract_reference(ref_text);
            }
            
            // Extract case material
            if let Some(material) = details.get("case_material_table") {
                watch.case_material = clean_text(material);
            }
            
            // Extract diameter
            if let Some(diameter) = details.get("diameter_table") {
                watch.diameter = clean_text(diameter);
            }
        }
    }
    
    // Try to extract brand from known brands if not found
    if watch.brand == "❓" && watch.title != "❓" {
        let title_lower = watch.title.to_lowercase();
        
        // Sort by length descending to match longest brand names first
        let mut sorted_brands: Vec<&str> = KNOWN_BRANDS.to_vec();
        sorted_brands.sort_by_key(|b| std::cmp::Reverse(b.len()));
        
        // First try: exact start match
        for brand in &sorted_brands {
            if title_lower.starts_with(&brand.to_lowercase()) {
                watch.brand = brand.to_string();
                break;
            }
        }
        
        // Second try: contains match
        if watch.brand == "❓" {
            for brand in &sorted_brands {
                if title_lower.contains(&brand.to_lowercase()) {
                    watch.brand = brand.to_string();
                    break;
                }
            }
        }
    }
    
    // Extract model from title if not found
    if watch.model == "❓" && watch.brand != "❓" && watch.title != "❓" {
        // Remove brand from title
        let re = Regex::new(&format!(r"(?i)^{}\s*", regex::escape(&watch.brand))).unwrap();
        let mut temp_model = re.replace(&watch.title, "").to_string();
        
        // Remove year if found
        if watch.year != "❓" {
            temp_model = temp_model.replace(&watch.year, "").trim().to_string();
        }
        
        // Remove reference if found
        if watch.reference != "❓" {
            let ref_re = Regex::new(&format!(r"(?i){}", regex::escape(&watch.reference))).unwrap();
            temp_model = ref_re.replace(&temp_model, "").trim().to_string();
        }
        
        // Take first 3 words excluding 4-digit numbers
        let words: Vec<&str> = temp_model.split_whitespace()
            .filter(|w| !(w.len() == 4 && w.chars().all(|c| c.is_numeric())))
            .take(3)
            .collect();
        
        if !words.is_empty() {
            let model_candidate = words.join(" ");
            if model_candidate.to_lowercase() != watch.brand.to_lowercase() {
                watch.model = model_candidate;
            }
        }
    }
    
    // Extract reference from title if not found
    if watch.reference == "❓" && watch.title != "❓" {
        let mut temp_ref_str = watch.title.clone();
        
        // Remove brand
        if watch.brand != "❓" {
            let re = Regex::new(&format!(r"(?i){}", regex::escape(&watch.brand))).unwrap();
            temp_ref_str = re.replace(&temp_ref_str, "").to_string();
        }
        
        // Remove model
        if watch.model != "❓" {
            let re = Regex::new(&format!(r"(?i){}", regex::escape(&watch.model))).unwrap();
            temp_ref_str = re.replace(&temp_ref_str, "").to_string();
        }
        
        // Remove year
        if watch.year != "❓" {
            temp_ref_str = temp_ref_str.replace(&watch.year, "");
        }
        
        // Look for reference pattern
        let ref_re = Regex::new(r"\b([A-Z0-9]{3,}(?:[-/\s]?[A-Z0-9]+)?)\b").unwrap();
        if let Some(cap) = ref_re.captures(&temp_ref_str) {
            if let Some(m) = cap.get(1) {
                watch.reference = m.as_str().to_string();
            }
        }
    }
}