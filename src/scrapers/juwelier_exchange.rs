use async_trait::async_trait;
use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};
use url::Url;

use crate::config::{Config, SiteConfig};
use crate::models::{Site, WatchListing, BoxStatus, PapersStatus};
use crate::parsers::{clean_text, format_price_eur_display, get_price_string_for_hash, 
                      parse_year_from_string, parse_box_papers_status, get_condition_display,
                      extract_reference};
use crate::scrapers::WatchScraper;
use crate::utils::http::fetch_with_retry;

pub struct JuwelierExchangeScraper {
    config: Arc<Config>,
}

impl JuwelierExchangeScraper {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[derive(Clone)]
struct WatchData {
    url: String,
    image_url: String,
    price_raw: String,
    price_display: String,
}

#[derive(Default)]
struct DetailPageData {
    title: String,
    brand: String,
    model: String,
    reference: String,
    year: String,
    condition_text: String,
    case_material: String,
    diameter: String,
    box_status: String,
    papers_status: String,
    description_main: String,
}

#[async_trait]
impl WatchScraper for JuwelierExchangeScraper {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>> {
        let site_config = self.site_config();
        info!("Scraping Juwelier Exchange...");
        
        let response = fetch_with_retry(client, &site_config.url, 3).await?;
        let html = response.text().await?;
        
        // Extract all data synchronously
        let watch_data = extract_watch_data(&html, &site_config.base_url)?;
        
        info!("Found {} watch items (product cards) on Juwelier Exchange listing page", watch_data.len());
        
        let mut listings = Vec::new();
        
        // Process each watch with async operations
        for data in watch_data {
            if !data.url.is_empty() && data.url != site_config.base_url {
                match self.process_watch(data, client, site_config).await {
                    Ok(listing) => listings.push(listing),
                    Err(e) => error!("Error parsing Juwelier Exchange item: {}", e),
                }
            }
        }
        
        Ok(listings)
    }
    
    fn site_config(&self) -> &SiteConfig {
        &self.config.sites["juwelier_exchange"]
    }
    
    fn site_key(&self) -> Site {
        Site::JuwelierExchange
    }
}

fn extract_watch_data(html: &str, base_url: &str) -> Result<Vec<WatchData>> {
    let document = Html::parse_document(html);
    let card_selector = Selector::parse("div.card.product-box[data-product-information]")
        .map_err(|_| anyhow::anyhow!("Failed to parse card selector"))?;
    
    let mut watch_data = Vec::new();
    
    for element in document.select(&card_selector) {
        let mut data = WatchData {
            url: String::new(),
            image_url: String::new(),
            price_raw: String::new(),
            price_display: String::new(),
        };
        
        // Extract link
        if let Ok(link_selector) = Selector::parse("a.card-body-link") {
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
        
        // Extract image with complex srcset logic
        if let Ok(img_selector) = Selector::parse("img.product-image") {
            if let Some(img) = element.select(&img_selector).next() {
                let srcset = img.value().attr("srcset").unwrap_or("");
                if !srcset.is_empty() {
                    // Parse srcset and prefer higher resolution webp
                    let potential_srcs: Vec<&str> = srcset.split(',')
                        .map(|s| s.trim().split_whitespace().next().unwrap_or(""))
                        .collect();
                    
                    let mut best_src = img.value().attr("src").unwrap_or("");
                    
                    // Order of preference for resolution
                    let resolutions = ["1920x1920.webp", "800x800.webp", "400x400.webp", ".webp"];
                    for res in resolutions {
                        for p_src in &potential_srcs {
                            if p_src.contains(res) {
                                best_src = p_src;
                                break;
                            }
                        }
                        if best_src.contains(res) {
                            break;
                        }
                    }
                    
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(full_url) = base.join(best_src) {
                            data.image_url = full_url.to_string();
                        }
                    }
                } else if let Some(src) = img.value().attr("src") {
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(full_url) = base.join(src) {
                            data.image_url = full_url.to_string();
                        }
                    }
                }
            }
        }
        
        // Extract price from listing
        if let Ok(price_selector) = Selector::parse("span.product-price") {
            if let Some(price_elem) = element.select(&price_selector).next() {
                let price_text = clean_text(&price_elem.text().collect::<String>());
                data.price_raw = get_price_string_for_hash(&price_text);
                data.price_display = format_price_eur_display(&price_text);
            }
        }
        
        watch_data.push(data);
    }
    
    Ok(watch_data)
}

impl JuwelierExchangeScraper {
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
            price_eur_raw_for_hash: data.price_raw,
            price_eur_display: data.price_display,
            ..Default::default()
        };
        
        // Fetch detail page for additional information
        info!("Fetching details for Juwelier Exchange item (URL: {})", data.url);
        
        // Add delay to be respectful (slightly longer for complex pages)
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        
        match fetch_with_retry(client, &data.url, 3).await {
            Ok(detail_response) => {
                let detail_html = detail_response.text().await?;
                let details = parse_detail_page(&detail_html);
                
                // Update watch with detail page data
                if !details.title.is_empty() {
                    watch.title = details.title;
                }
                watch.brand = if !details.brand.is_empty() { details.brand } else { "❓".to_string() };
                watch.model = if !details.model.is_empty() { details.model } else { "❓".to_string() };
                watch.reference = if !details.reference.is_empty() { details.reference } else { "❓".to_string() };
                watch.year = if !details.year.is_empty() { details.year } else { "❓".to_string() };
                
                // Handle condition
                let all_desc_texts = vec![details.description_main.clone(), details.condition_text.clone()];
                watch.condition_display = get_condition_display(
                    &details.condition_text,
                    Site::JuwelierExchange,
                    Some(&all_desc_texts)
                );
                
                // Parse box/papers status
                let (papers, box_status) = parse_box_papers_status(&details.description_main);
                watch.papers_status = if details.papers_status == "✅" { PapersStatus::Yes } else { papers };
                watch.box_status = if details.box_status == "✅" { BoxStatus::Yes } else { box_status };
                
                watch.case_material = if !details.case_material.is_empty() { details.case_material } else { "❓".to_string() };
                watch.diameter = if !details.diameter.is_empty() { details.diameter } else { "❓".to_string() };
            }
            Err(e) => {
                error!("Could not fetch detail page for {}: {}", data.url, e);
                
                // Try to extract some info from listing page description if detail fetch failed
                // This would require passing listing HTML element data, skipping for now
            }
        }
        
        Ok(watch)
    }
}

fn parse_detail_page(html: &str) -> DetailPageData {
    let document = Html::parse_document(html);
    let mut details = DetailPageData::default();
    
    // Parse JSON-LD data
    if let Ok(script_selector) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for script in document.select(&script_selector) {
            let script_text = script.text().collect::<String>();
            if script_text.contains(r#""@type": "Product""#) || script_text.contains(r#""@type":"Product""#) {
                if let Ok(json_data) = serde_json::from_str::<Value>(&script_text) {
                    // Extract from JSON-LD
                    if let Some(name) = json_data.get("name").and_then(|v| v.as_str()) {
                        details.title = clean_text(name);
                    }
                    
                    if let Some(brand) = json_data.get("brand") {
                        if let Some(brand_name) = brand.get("name").and_then(|v| v.as_str()) {
                            details.brand = clean_text(brand_name);
                        }
                    }
                    
                    if let Some(sku) = json_data.get("sku").and_then(|v| v.as_str()) {
                        details.reference = clean_text(sku);
                    }
                    
                    if let Some(desc) = json_data.get("description").and_then(|v| v.as_str()) {
                        details.description_main = clean_text(desc);
                    }
                    
                    // Check item condition
                    if let Some(offers) = json_data.get("offers") {
                        if let Some(condition) = offers.get("itemCondition").and_then(|v| v.as_str()) {
                            if condition.contains("NewCondition") {
                                details.condition_text = "Neu".to_string();
                            } else if condition.contains("UsedCondition") {
                                details.condition_text = "Gebraucht".to_string();
                            } else if condition.contains("RefurbishedCondition") {
                                details.condition_text = "Aufgearbeitet".to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Override/supplement with visible elements if JSON-LD is incomplete
    if let Ok(title_selector) = Selector::parse("h1.product-detail-name") {
        if let Some(title_elem) = document.select(&title_selector).next() {
            let title_text = clean_text(&title_elem.text().collect::<String>());
            if details.title.is_empty() || details.title == "❓" {
                details.title = title_text;
            }
        }
    }
    
    // Parse properties table
    if let Ok(table_selector) = Selector::parse("table.product-detail-properties-table") {
        if let Some(table) = document.select(&table_selector).next() {
            if let Ok(row_selector) = Selector::parse("tr.properties-row") {
                for row in table.select(&row_selector) {
                    if let (Ok(label_sel), Ok(value_sel)) = (
                        Selector::parse("th.properties-label"),
                        Selector::parse("td.properties-value")
                    ) {
                        if let (Some(label_elem), Some(value_elem)) = (
                            row.select(&label_sel).next(),
                            row.select(&value_sel).next()
                        ) {
                            let label = clean_text(&label_elem.text().collect::<String>())
                                .to_lowercase()
                                .replace(":", "");
                            let value = clean_text(&value_elem.text().collect::<String>());
                            
                            match label.as_str() {
                                "artikelnummer" => {
                                    if details.reference.is_empty() || details.reference == "❓" {
                                        details.reference = value;
                                    }
                                }
                                "marke" => {
                                    if details.brand.is_empty() || details.brand == "❓" {
                                        details.brand = value;
                                    }
                                }
                                "zustand" => {
                                    if details.condition_text.is_empty() || details.condition_text == "❓" {
                                        details.condition_text = value;
                                    }
                                }
                                "art der legierung" => {
                                    details.case_material = value;
                                }
                                "legierung" => {
                                    if value.chars().all(|c| c.is_numeric()) && !details.case_material.is_empty() {
                                        details.case_material = format!("{} {}", value, details.case_material);
                                    }
                                }
                                "material" => {
                                    if details.case_material.is_empty() || details.case_material == "❓" {
                                        details.case_material = value;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Parse main description
    if let Ok(desc_selector) = Selector::parse(r#"div.product-detail-description-text[itemprop="description"]"#) {
        if let Some(desc_elem) = document.select(&desc_selector).next() {
            let full_desc = clean_text(&desc_elem.text().collect::<String>());
            if details.description_main.is_empty() || details.description_main == "❓" {
                details.description_main = full_desc.clone();
            }
            
            // Extract year from description
            if details.year.is_empty() || details.year == "❓" {
                details.year = parse_year_from_string(&full_desc, Some(&details.title));
            }
            
            // Extract box/papers info
            let (papers, box_status) = parse_box_papers_status(&full_desc);
            details.papers_status = match papers {
                PapersStatus::Yes => "✅".to_string(),
                PapersStatus::No => "❌".to_string(),
                PapersStatus::Unknown => "❓".to_string(),
            };
            details.box_status = match box_status {
                BoxStatus::Yes => "✅".to_string(),
                BoxStatus::No => "❌".to_string(),
                BoxStatus::Unknown => "❓".to_string(),
            };
            
            // Extract diameter
            let diameter_re = Regex::new(r"(\d{2,3})\s*mm").unwrap();
            if let Some(cap) = diameter_re.captures(&full_desc) {
                if let Some(m) = cap.get(1) {
                    details.diameter = format!("{} mm", m.as_str());
                }
            }
        }
    }
    
    // Try to infer model from title and brand
    if !details.title.is_empty() && !details.brand.is_empty() && details.model.is_empty() {
        let title_without_brand = details.title.replace(&details.brand, "").trim().to_string();
        let words: Vec<&str> = title_without_brand.split_whitespace().take(3).collect();
        if !words.is_empty() {
            details.model = words.join(" ");
        }
    }
    
    details
}