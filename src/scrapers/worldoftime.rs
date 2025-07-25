use async_trait::async_trait;
use anyhow::Result;
use reqwest::Client;
use scraper::{Html, Selector};
use std::sync::Arc;
use tracing::{error, info};
use url::Url;

use crate::config::{Config, SiteConfig};
use crate::models::Site;
use crate::models::WatchListing;
use crate::parsers::{clean_text, format_price_eur_display, get_price_string_for_hash, 
                      parse_year_from_string, parse_box_papers_status, get_condition_display,
                      extract_reference, parse_table_th_td};
use crate::scrapers::WatchScraper;
use crate::utils::http::fetch_with_retry;

pub struct WorldOfTimeScraper {
    config: Arc<Config>,
}

impl WorldOfTimeScraper {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[derive(Clone)]
struct WatchData {
    url: String,
    title: String,
    brand: String,
    model: String,
    price_raw: String,
    price_display: String,
    image_url: String,
}

#[async_trait]
impl WatchScraper for WorldOfTimeScraper {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>> {
        let site_config = self.site_config();
        info!("Scraping World of Time...");
        
        let response = fetch_with_retry(client, &site_config.url, 3).await?;
        let html = response.text().await?;
        
        // Extract all data synchronously
        let watch_data = extract_watch_data(&html, &site_config.base_url)?;
        
        info!("Found {} watch items on World of Time listing page", watch_data.len());
        
        let mut listings = Vec::new();
        
        // Process each watch with async operations
        for data in watch_data {
            match self.process_watch(data, client, site_config).await {
                Ok(listing) => listings.push(listing),
                Err(e) => error!("Error parsing World of Time item: {}", e),
            }
        }
        
        Ok(listings)
    }
    
    fn site_config(&self) -> &SiteConfig {
        &self.config.sites["worldoftime"]
    }
    
    fn site_key(&self) -> Site {
        Site::WorldOfTime
    }
}

fn extract_watch_data(html: &str, base_url: &str) -> Result<Vec<WatchData>> {
    let document = Html::parse_document(html);
    let watch_selector = Selector::parse("div.new-arrivals-watch, div.paged-clocks-container div.watch-link")
        .map_err(|_| anyhow::anyhow!("Failed to parse watch selector"))?;
    
    let mut watch_data = Vec::new();
    
    for element in document.select(&watch_selector) {
        let mut data = WatchData {
            url: String::new(),
            title: String::new(),
            brand: String::new(),
            model: String::new(),
            price_raw: String::new(),
            price_display: String::new(),
            image_url: String::new(),
        };
        
        // Extract link
        if let Ok(link_selector) = Selector::parse("a") {
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
        
        // Extract image
        if let Ok(img_selector) = Selector::parse("img") {
            if let Some(img) = element.select(&img_selector).next() {
                if let Some(src) = img.value().attr("src").or_else(|| img.value().attr("data-src")) {
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(full_url) = base.join(src) {
                            data.image_url = full_url.to_string();
                        }
                    }
                }
            }
        }
        
        // Extract title and brand
        if let Ok(title_selector) = Selector::parse("h2, .watch-title") {
            if let Some(title_elem) = element.select(&title_selector).next() {
                let full_title = clean_text(&title_elem.text().collect::<String>());
                data.title = full_title.clone();
                
                // Try to extract brand from title (first word usually)
                let parts: Vec<&str> = full_title.split_whitespace().collect();
                if !parts.is_empty() {
                    data.brand = parts[0].to_string();
                    
                    // Model is the rest after brand
                    if parts.len() > 1 {
                        data.model = parts[1..].join(" ");
                    }
                }
            }
        }
        
        // Extract price
        if let Ok(price_selector) = Selector::parse(".watch-price, .price") {
            if let Some(price_elem) = element.select(&price_selector).next() {
                let price_text = clean_text(&price_elem.text().collect::<String>());
                data.price_raw = get_price_string_for_hash(&price_text);
                data.price_display = format_price_eur_display(&price_text);
            }
        }
        
        if !data.url.is_empty() {
            watch_data.push(data);
        }
    }
    
    Ok(watch_data)
}

impl WorldOfTimeScraper {
    async fn process_watch(
        &self,
        data: WatchData,
        client: &Client,
        site_config: &SiteConfig,
    ) -> Result<WatchListing> {
        let mut watch = WatchListing {
            site_name: site_config.name.clone(),
            watch_url: data.url.clone(),
            image_url: data.image_url,
            title: data.title,
            brand: data.brand,
            model: data.model,
            price_eur_raw_for_hash: data.price_raw,
            price_eur_display: data.price_display,
            ..Default::default()
        };
        
        // Fetch additional details
        if !data.url.is_empty() {
            info!("Fetching details for World of Time item (URL: {})", data.url);
            
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
        }
        
        Ok(watch)
    }
}

fn parse_detail_page(html: &str, watch: &mut WatchListing) {
    let document = Html::parse_document(html);
    
    // Try to find more detailed title/brand info
    if let Ok(h1_selector) = Selector::parse("h1") {
        if let Some(h1) = document.select(&h1_selector).next() {
            let detailed_title = clean_text(&h1.text().collect::<String>());
            if !detailed_title.is_empty() {
                watch.title = detailed_title.clone();
                
                // Re-parse brand/model with better title
                let parts: Vec<&str> = detailed_title.split_whitespace().collect();
                if !parts.is_empty() {
                    watch.brand = parts[0].to_string();
                    if parts.len() > 1 {
                        watch.model = parts[1..].join(" ");
                    }
                }
            }
        }
    }
    
    // Look for details table
    if let Ok(table_selector) = Selector::parse("table.details-table, table.product-details") {
        if let Some(table) = document.select(&table_selector).next() {
            let table_html = table.html();
            let headers_map = std::collections::HashMap::from([
                ("Referenz", "reference"),
                ("Reference", "reference"),
                ("Jahr", "year"),
                ("Year", "year"),
                ("Zustand", "condition"),
                ("Condition", "condition"),
                ("Gehäuse", "case_material"),
                ("Case", "case_material"),
                ("Durchmesser", "diameter"),
                ("Diameter", "diameter"),
                ("Lieferumfang", "scope"),
                ("Scope of delivery", "scope"),
            ]);
            
            let details = parse_table_th_td(&table_html, &headers_map);
            
            if let Some(ref_val) = details.get("reference") {
                watch.reference = extract_reference(ref_val);
            }
            
            if let Some(year_val) = details.get("year") {
                watch.year = parse_year_from_string(year_val, Some(&watch.title));
            }
            
            if let Some(condition_val) = details.get("condition") {
                watch.condition_display = get_condition_display(condition_val, Site::WorldOfTime, None);
            }
            
            if let Some(material) = details.get("case_material") {
                watch.case_material = clean_text(material);
            }
            
            if let Some(diameter) = details.get("diameter") {
                watch.diameter = clean_text(diameter);
            }
            
            if let Some(scope) = details.get("scope") {
                let (papers, box_status) = parse_box_papers_status(scope);
                watch.papers_status = papers;
                watch.box_status = box_status;
            }
        }
    }
}