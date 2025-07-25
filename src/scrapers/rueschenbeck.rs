use async_trait::async_trait;
use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector, ElementRef};
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

pub struct RueschenbeckScraper {
    config: Arc<Config>,
}

impl RueschenbeckScraper {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[derive(Clone)]
struct WatchData {
    url: String,
    image_url: String,
    brand: String,
    model: String,
    title: String,
    reference: String,
    price_raw: String,
    price_display: String,
    is_cpo: bool,
}

#[derive(Default)]
struct DetailPageData {
    year_text: String,
    reference_text: String,
    diameter_text: String,
    case_material_text: String,
    condition_text: String,
    packaging_text: String,
    papers_text: String,
    papiere_direct_confirm: bool,
}

#[async_trait]
impl WatchScraper for RueschenbeckScraper {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>> {
        let site_config = self.site_config();
        info!("Scraping Rüschenbeck...");
        
        let response = fetch_with_retry(client, &site_config.url, 3).await?;
        let html = response.text().await?;
        
        // Extract all data synchronously
        let watch_data = extract_watch_data(&html, &site_config.base_url)?;
        
        info!("Found {} watch items on Rüschenbeck listing page", watch_data.len());
        
        let mut listings = Vec::new();
        
        // Process each watch with async operations
        for data in watch_data {
            if !data.url.is_empty() && data.url != site_config.base_url {
                match self.process_watch(data, client, site_config).await {
                    Ok(listing) => listings.push(listing),
                    Err(e) => error!("Error parsing Rüschenbeck item: {}", e),
                }
            }
        }
        
        Ok(listings)
    }
    
    fn site_config(&self) -> &SiteConfig {
        &self.config.sites["rueschenbeck"]
    }
    
    fn site_key(&self) -> Site {
        Site::Rueschenbeck
    }
}

fn extract_watch_data(html: &str, base_url: &str) -> Result<Vec<WatchData>> {
    let document = Html::parse_document(html);
    let item_selector = Selector::parse("li.-rb-list-item")
        .map_err(|_| anyhow::anyhow!("Failed to parse item selector"))?;
    
    let mut watch_data = Vec::new();
    
    for element in document.select(&item_selector) {
        // Check if sold out
        if let Ok(avail_selector) = Selector::parse(".-rb-availability .out-of-stock span.value, .-rb-availability .sold span.value") {
            if let Some(avail_elem) = element.select(&avail_selector).next() {
                let avail_text = clean_text(&avail_elem.text().collect::<String>()).to_lowercase();
                if avail_text.contains("verkauft") {
                    continue; // Skip sold items
                }
            }
        }
        
        let mut data = WatchData {
            url: String::new(),
            image_url: String::new(),
            brand: String::new(),
            model: String::new(),
            title: String::new(),
            reference: String::new(),
            price_raw: String::new(),
            price_display: String::new(),
            is_cpo: false,
        };
        
        // Extract link
        if let Ok(link_selector) = Selector::parse("a.-rb-list-item-link") {
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
        if let Ok(img_selector) = Selector::parse(".-rb-list-image img") {
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
        
        // Extract brand
        if let Ok(brand_selector) = Selector::parse("span.-rb-manufacturer-name") {
            if let Some(brand_elem) = element.select(&brand_selector).next() {
                data.brand = clean_text(&brand_elem.text().collect::<String>());
            }
        }
        
        // Extract model
        if let Ok(model_selector) = Selector::parse("span.-rb-line-name") {
            if let Some(model_elem) = element.select(&model_selector).next() {
                data.model = clean_text(&model_elem.text().collect::<String>());
            }
        }
        
        // Extract full title and reference from product name
        if let Ok(prod_selector) = Selector::parse("span.-rb-prod-name") {
            if let Some(prod_elem) = element.select(&prod_selector).next() {
                data.title = clean_text(&prod_elem.text().collect::<String>());
                
                // Extract reference from beginning of title
                let ref_re = Regex::new(r"^([A-Za-z0-9\\-./]+)").unwrap();
                if let Some(cap) = ref_re.captures(&data.title) {
                    if let Some(m) = cap.get(1) {
                        let potential_ref = m.as_str();
                        // Filter out common non-reference words
                        if !potential_ref.to_lowercase().eq("certified") && 
                           !(potential_ref.chars().all(|c| c.is_numeric()) && potential_ref.len() < 4) {
                            data.reference = potential_ref.to_string();
                        }
                    }
                }
            }
        }
        
        // Extract price
        if let Ok(price_box_selector) = Selector::parse(".price-box") {
            if let Some(price_box) = element.select(&price_box_selector).next() {
                let mut price_text = String::new();
                
                // Try special price first
                if let Ok(special_selector) = Selector::parse("p.special-price span.price") {
                    if let Some(special_elem) = price_box.select(&special_selector).next() {
                        price_text = clean_text(&special_elem.text().collect::<String>());
                    }
                }
                
                // Fallback to regular price
                if price_text.is_empty() {
                    if let Ok(regular_selector) = Selector::parse("span.regular-price span.price") {
                        if let Some(regular_elem) = price_box.select(&regular_selector).next() {
                            price_text = clean_text(&regular_elem.text().collect::<String>());
                        }
                    }
                }
                
                if !price_text.is_empty() {
                    data.price_raw = get_price_string_for_hash(&price_text);
                    data.price_display = format_price_eur_display(&price_text);
                }
            }
        }
        
        // Check for CPO (Certified Pre-Owned) status
        if let Ok(cpo_selector) = Selector::parse("span.-rb-icon.icn-cpo") {
            if element.select(&cpo_selector).next().is_some() {
                data.is_cpo = true;
            }
        }
        
        watch_data.push(data);
    }
    
    Ok(watch_data)
}

impl RueschenbeckScraper {
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
            brand: data.brand,
            model: data.model,
            title: data.title,
            reference: data.reference,
            price_eur_raw_for_hash: data.price_raw,
            price_eur_display: data.price_display,
            ..Default::default()
        };
        
        // Set CPO condition if applicable
        if data.is_cpo {
            watch.condition_display = "★★★★☆".to_string(); // 4 stars for CPO
        }
        
        // Fetch detail page for additional information
        info!("Fetching details for Rüschenbeck item: {} (URL: {})", 
              if !watch.title.is_empty() { &watch.title } else { "N/A" }, 
              data.url);
        
        // Add delay to be respectful
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        
        match fetch_with_retry(client, &data.url, 3).await {
            Ok(detail_response) => {
                let detail_html = detail_response.text().await?;
                let details = parse_detail_page(&detail_html);
                
                // Update watch with detail page data
                if !details.year_text.is_empty() {
                    let year = parse_year_from_string(&details.year_text, None);
                    if year != "❓" {
                        watch.year = year;
                    }
                }
                
                // Update reference if detail page has more info
                if !details.reference_text.is_empty() && 
                   (watch.reference.is_empty() || watch.reference == "❓" || 
                    details.reference_text.len() > watch.reference.len()) {
                    watch.reference = details.reference_text.trim().to_string();
                }
                
                // Parse diameter
                if !details.diameter_text.is_empty() {
                    let dia_re = Regex::new(r"(\\d{1,2}(?:[.,]\\d{1,2})?)\\s*mm").unwrap();
                    if let Some(cap) = dia_re.captures(&details.diameter_text) {
                        if let Some(m) = cap.get(1) {
                            watch.diameter = format!("{} mm", m.as_str().replace(",", "."));
                        }
                    } else {
                        // Try cleaning and formatting
                        let cleaned = details.diameter_text
                            .replace("mm", "")
                            .trim()
                            .replace(",", ".")
                            .replace(" ", "");
                        if Regex::new(r"^\\d+(\\.\\d+)?$").unwrap().is_match(&cleaned) {
                            watch.diameter = format!("{} mm", cleaned);
                        } else {
                            watch.diameter = details.diameter_text.clone();
                        }
                    }
                }
                
                // Set case material
                if !details.case_material_text.is_empty() {
                    // Title case the material
                    watch.case_material = details.case_material_text
                        .split_whitespace()
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                }
                
                // Parse box/papers status
                let combined_text = vec![
                    details.packaging_text.clone(),
                    details.papers_text.clone()
                ];
                let (papers, box_status) = parse_box_papers_status(&combined_text.join(" "));
                
                // Override papers status if directly confirmed
                watch.papers_status = if details.papiere_direct_confirm {
                    PapersStatus::Yes
                } else {
                    papers
                };
                watch.box_status = box_status;
                
                // Get condition if not already set from CPO
                if watch.condition_display == "❓" || watch.condition_display.is_empty() {
                    watch.condition_display = get_condition_display(
                        &details.condition_text,
                        Site::Rueschenbeck,
                        None
                    );
                }
            }
            Err(e) => {
                error!("Could not fetch detail page for {}: {}", data.url, e);
            }
        }
        
        Ok(watch)
    }
}

fn parse_detail_page(html: &str) -> DetailPageData {
    let document = Html::parse_document(html);
    let mut details = DetailPageData::default();
    
    // Parse CPO info section
    if let Ok(cpo_selector) = Selector::parse("div.additional-info-cpo") {
        if let Some(cpo_section) = document.select(&cpo_selector).next() {
            if let Ok(p_selector) = Selector::parse("p") {
                for p_elem in cpo_section.select(&p_selector) {
                    if let Ok(strong_selector) = Selector::parse("strong") {
                        if let Some(strong_elem) = p_elem.select(&strong_selector).next() {
                            let key = clean_text(&strong_elem.text().collect::<String>())
                                .to_lowercase()
                                .replace(":", "");
                            
                            // Extract value from span.data elements
                            let mut value_parts = Vec::new();
                            if let Ok(data_selector) = Selector::parse("span.data") {
                                for data_span in p_elem.select(&data_selector) {
                                    let text = clean_text(&data_span.text().collect::<String>());
                                    if !text.is_empty() {
                                        value_parts.push(text);
                                    }
                                }
                            }
                            
                            let value = if !value_parts.is_empty() {
                                value_parts.join(" | ")
                            } else {
                                // Fallback: get all text except the strong tag
                                let full_text = clean_text(&p_elem.text().collect::<String>());
                                let strong_text = clean_text(&strong_elem.text().collect::<String>());
                                full_text.replace(&strong_text, "").trim().to_string()
                            };
                            
                            match key.as_str() {
                                "jahr" => details.year_text = value,
                                "zustand" => details.condition_text = value,
                                "verpackung" => details.packaging_text = value,
                                "papiere" => details.papers_text = value,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Parse additional info section
    if let Ok(info_selector) = Selector::parse("div.additional-info div.rolex-textwrapper") {
        if let Some(info_section) = document.select(&info_selector).next() {
            if let Ok(p_selector) = Selector::parse(r#"p[class*="attr-"]"#) {
                for p_elem in info_section.select(&p_selector) {
                    if let (Ok(strong_sel), Ok(data_sel)) = (
                        Selector::parse("strong"),
                        Selector::parse("span.data")
                    ) {
                        if let (Some(strong_elem), Some(data_elem)) = (
                            p_elem.select(&strong_sel).next(),
                            p_elem.select(&data_sel).next()
                        ) {
                            let key = clean_text(&strong_elem.text().collect::<String>())
                                .to_lowercase()
                                .replace(":", "");
                            let value = clean_text(&data_elem.text().collect::<String>());
                            
                            match key.as_str() {
                                "referenz" => details.reference_text = value,
                                "durchmesser" => details.diameter_text = value,
                                "gehäuse" => details.case_material_text = value,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Check for direct papers confirmation
    details.papiere_direct_confirm = html.contains("Papiere: Ja") || 
                                    html.contains("Papers: Yes") ||
                                    (details.papers_text.to_lowercase() == "ja");
    
    details
}