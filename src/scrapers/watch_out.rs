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
use crate::models::{Site, WatchListing};
use crate::parsers::{clean_text, format_price_eur_display, get_price_string_for_hash, 
                      parse_year_from_string, parse_box_papers_status, get_condition_display};
use crate::scrapers::WatchScraper;
use crate::utils::http::fetch_with_retry;

pub struct WatchOutScraper {
    config: Arc<Config>,
}

impl WatchOutScraper {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[derive(Clone, Default)]
struct WatchData {
    url: String,
    title: String,
    brand: String,
    reference: String,
    price_raw: String,
    price_display: String,
    image_url: String,
    handle: String,
}

#[derive(Debug, Clone)]
struct ShopifyProduct {
    title: String,
    brand: String,
    reference: String,
    price_cents: Option<i64>,
    url_part: String,
}

#[async_trait]
impl WatchScraper for WatchOutScraper {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>> {
        let site_config = self.site_config();
        info!("Scraping Watch Out...");
        
        let response = fetch_with_retry(client, &site_config.url, 3).await?;
        let html = response.text().await?;
        
        // Extract Shopify analytics data and product cards
        let (shopify_products, watch_data) = extract_watch_data(&html, &site_config.base_url)?;
        
        info!("Found {} product-card elements on Watch Out page", watch_data.len());
        if !shopify_products.is_empty() {
            info!("Found {} items in Watch Out ShopifyAnalytics data", shopify_products.len());
        }
        
        let mut listings = Vec::new();
        
        // Process each watch with async operations
        for (idx, mut data) in watch_data.into_iter().enumerate() {
            // Try to match with Shopify data
            if idx < shopify_products.len() {
                let shopify = &shopify_products[idx];
                
                // Match by handle or title
                if (!data.handle.is_empty() && shopify.url_part.contains(&data.handle)) ||
                   (!data.title.is_empty() && data.title.to_lowercase().contains(&shopify.title.to_lowercase())) ||
                   (data.handle.is_empty() && data.title.is_empty()) {
                    
                    // Use Shopify data to supplement
                    if data.brand.is_empty() || data.brand == "❓" {
                        data.brand = shopify.brand.clone();
                    }
                    if !shopify.title.is_empty() && shopify.title.to_lowercase() != "default title" {
                        data.title = shopify.title.clone();
                    }
                    if data.reference.is_empty() || data.reference == "❓" {
                        data.reference = shopify.reference.clone();
                    }
                    if let Some(price_cents) = shopify.price_cents {
                        data.price_raw = price_cents.to_string();
                        data.price_display = format_price_eur_display(&(price_cents as f64 / 100.0).to_string());
                    }
                }
            }
            
            if !data.url.is_empty() {
                match self.process_watch(data, client, site_config).await {
                    Ok(listing) => listings.push(listing),
                    Err(e) => error!("Error parsing Watch Out item: {}", e),
                }
            }
        }
        
        Ok(listings)
    }
    
    fn site_config(&self) -> &SiteConfig {
        &self.config.sites["watch_out"]
    }
    
    fn site_key(&self) -> Site {
        Site::WatchOut
    }
}

fn extract_watch_data(html: &str, base_url: &str) -> Result<(Vec<ShopifyProduct>, Vec<WatchData>)> {
    let document = Html::parse_document(html);
    let mut shopify_products = Vec::new();
    
    // Try to extract Shopify analytics data
    if let Ok(script_selector) = Selector::parse("script") {
        for script in document.select(&script_selector) {
            let script_text = script.text().collect::<String>();
            if script_text.contains("window.ShopifyAnalytics.meta") {
                // Look for var meta = {...}
                let re = Regex::new(r"var meta = (\{.*?\});").unwrap();
                if let Some(cap) = re.captures(&script_text) {
                    if let Some(json_str) = cap.get(1) {
                        match serde_json::from_str::<Value>(json_str.as_str()) {
                            Ok(meta_data) => {
                                if let Some(products) = meta_data.get("products").and_then(|p| p.as_array()) {
                                    for product in products {
                                        let mut shopify_product = ShopifyProduct {
                                            title: String::new(),
                                            brand: String::new(),
                                            reference: String::new(),
                                            price_cents: None,
                                            url_part: String::new(),
                                        };
                                        
                                        // Extract vendor (brand)
                                        if let Some(vendor) = product.get("vendor").and_then(|v| v.as_str()) {
                                            shopify_product.brand = clean_text(vendor);
                                        }
                                        
                                        // Extract title from variant or product
                                        if let Some(variants) = product.get("variants").and_then(|v| v.as_array()) {
                                            if let Some(first_variant) = variants.first() {
                                                if let Some(name) = first_variant.get("name").and_then(|n| n.as_str()) {
                                                    shopify_product.title = clean_text(name);
                                                }
                                                
                                                // Extract price
                                                if let Some(price) = first_variant.get("price").and_then(|p| p.as_i64()) {
                                                    shopify_product.price_cents = Some(price);
                                                }
                                                
                                                // Extract SKU as reference
                                                if let Some(sku) = first_variant.get("sku").and_then(|s| s.as_str()) {
                                                    shopify_product.reference = clean_text(sku);
                                                }
                                                
                                                // Extract URL part
                                                if let Some(variant_product) = first_variant.get("product") {
                                                    if let Some(url) = variant_product.get("url").and_then(|u| u.as_str()) {
                                                        shopify_product.url_part = url.to_string();
                                                    }
                                                }
                                            }
                                        }
                                        
                                        // Fallback to untranslatedTitle
                                        if shopify_product.title.is_empty() || shopify_product.title.to_lowercase() == "default title" {
                                            if let Some(title) = product.get("untranslatedTitle").and_then(|t| t.as_str()) {
                                                shopify_product.title = clean_text(title);
                                            } else if let Some(title) = product.get("title").and_then(|t| t.as_str()) {
                                                shopify_product.title = clean_text(title);
                                            }
                                        }
                                        
                                        shopify_products.push(shopify_product);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error parsing Watch Out ShopifyAnalytics data: {}", e);
                            }
                        }
                    }
                } else {
                    info!("Could not find 'var meta = {{...}}' in ShopifyAnalytics script for Watch Out.");
                }
            }
        }
    }
    
    // Extract product cards
    let mut watch_data = Vec::new();
    let card_selector = Selector::parse("product-card")
        .map_err(|_| anyhow::anyhow!("Failed to parse product-card selector"))?;
    
    for element in document.select(&card_selector) {
        let mut data = WatchData::default();
        
        // Check if sold out
        if let Ok(sold_selector) = Selector::parse("sold-out-badge") {
            if element.select(&sold_selector).next().is_some() {
                continue; // Skip sold out items
            }
        }
        
        // Extract handle attribute
        if let Some(handle) = element.value().attr("handle") {
            data.handle = handle.to_string();
            data.url = format!("{}/products/{}", base_url, handle);
        } else {
            // Try to find link
            if let Ok(link_selector) = Selector::parse(r#"a[href*="/products/"]"#) {
                if let Some(link) = element.select(&link_selector).next() {
                    if let Some(href) = link.value().attr("href") {
                        if let Ok(base) = Url::parse(base_url) {
                            if let Ok(full_url) = base.join(href) {
                                data.url = full_url.to_string();
                                // Extract handle from URL
                                if href.starts_with("/products/") {
                                    data.handle = href.split("/products/")
                                        .nth(1)
                                        .unwrap_or("")
                                        .split('?')
                                        .next()
                                        .unwrap_or("")
                                        .to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Extract title
        if let Ok(title_selector) = Selector::parse(".product-card__title a.bold") {
            if let Some(title_elem) = element.select(&title_selector).next() {
                data.title = clean_text(&title_elem.text().collect::<String>());
            }
        }
        
        // Extract brand
        if let Ok(brand_selector) = Selector::parse(".product-card__info a.text-xs.link-faded") {
            if let Some(brand_elem) = element.select(&brand_selector).next() {
                data.brand = clean_text(&brand_elem.text().collect::<String>());
            }
        }
        
        // Extract price
        if let Ok(price_selector) = Selector::parse("sale-price") {
            if let Some(price_elem) = element.select(&price_selector).next() {
                let price_text = clean_text(&price_elem.text().collect::<String>());
                data.price_raw = get_price_string_for_hash(&price_text);
                data.price_display = format_price_eur_display(&price_text);
            }
        }
        
        // Extract reference from badge
        if let Ok(ref_selector) = Selector::parse(".product-card__badge-list span.badge--primary") {
            if let Some(ref_elem) = element.select(&ref_selector).next() {
                let ref_text = clean_text(&ref_elem.text().collect::<String>());
                // Extract reference pattern
                let ref_re = Regex::new(r"\b([A-Z0-9]{3,}(?:[-/\s]?[A-Z0-9]+)?)\b").unwrap();
                if let Some(cap) = ref_re.captures(&ref_text) {
                    if let Some(m) = cap.get(1) {
                        data.reference = m.as_str().to_string();
                    }
                }
            }
        }
        
        // Extract image
        if let Ok(img_selector) = Selector::parse("img.product-card__image") {
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
        
        watch_data.push(data);
    }
    
    Ok((shopify_products, watch_data))
}

impl WatchOutScraper {
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
            title: data.title.clone(),
            brand: data.brand.clone(),
            reference: data.reference.clone(),
            price_eur_raw_for_hash: data.price_raw,
            price_eur_display: data.price_display,
            ..Default::default()
        };
        
        // Fetch detail page for additional information
        if !data.url.is_empty() {
            info!("Fetching details for Watch Out item (URL: {})", data.url);
            
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
    
    // Look for JSON-LD structured data
    if let Ok(script_selector) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for script in document.select(&script_selector) {
            let script_text = script.text().collect::<String>();
            if script_text.contains(r#""@type": "Product""#) || script_text.contains(r#""@type":"Product""#) {
                if let Ok(json_data) = serde_json::from_str::<Value>(&script_text) {
                    // Extract additional details from JSON-LD
                    if let Some(desc) = json_data.get("description").and_then(|d| d.as_str()) {
                        let description = clean_text(desc);
                        
                        // Extract year
                        if watch.year == "❓" || watch.year.is_empty() {
                            watch.year = parse_year_from_string(&description, Some(&watch.title));
                        }
                        
                        // Extract box/papers status
                        let (papers, box_status) = parse_box_papers_status(&description);
                        watch.papers_status = papers;
                        watch.box_status = box_status;
                        
                        // Get condition
                        watch.condition_display = get_condition_display("", Site::WatchOut, Some(&vec![description]));
                    }
                }
            }
        }
    }
    
    // Look for product details section
    if let Ok(details_selector) = Selector::parse(".product__details") {
        if let Some(details_elem) = document.select(&details_selector).next() {
            let details_text = clean_text(&details_elem.text().collect::<String>());
            
            // Extract year if not found
            if watch.year == "❓" || watch.year.is_empty() {
                watch.year = parse_year_from_string(&details_text, Some(&watch.title));
            }
            
            // Extract diameter
            let diameter_re = Regex::new(r"(\d{2,3})\s*mm").unwrap();
            if let Some(cap) = diameter_re.captures(&details_text) {
                if let Some(m) = cap.get(1) {
                    watch.diameter = format!("{} mm", m.as_str());
                }
            }
            
            // Extract case material patterns
            let material_patterns = [
                (r"stainless\s*steel", "Stainless Steel"),
                (r"(white|yellow|rose)\s*gold", "Gold"),
                (r"platinum", "Platinum"),
                (r"titanium", "Titanium"),
                (r"ceramic", "Ceramic"),
            ];
            
            for (pattern, material) in material_patterns {
                let re = Regex::new(&format!(r"(?i){}", pattern)).unwrap();
                if re.is_match(&details_text) {
                    watch.case_material = material.to_string();
                    break;
                }
            }
        }
    }
    
    // Try to extract model from title if not set
    if (watch.model == "❓" || watch.model.is_empty()) && !watch.brand.is_empty() && !watch.title.is_empty() {
        let title_without_brand = watch.title.replace(&watch.brand, "").trim().to_string();
        let words: Vec<&str> = title_without_brand.split_whitespace()
            .filter(|w| !w.chars().all(|c| c.is_numeric()))
            .take(3)
            .collect();
        if !words.is_empty() {
            watch.model = words.join(" ");
        }
    }
}