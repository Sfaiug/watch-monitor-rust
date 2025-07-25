use async_trait::async_trait;
use anyhow::Result;
use reqwest::Client;
use scraper::{Html, Selector};
use std::sync::Arc;
use tracing::{error, info};
use url::Url;

use crate::config::{Config, SiteConfig};
use crate::models::{Site, WatchListing};
use crate::parsers::{clean_text, format_price_eur_display, get_price_string_for_hash, 
                      parse_year_from_string, parse_box_papers_status, get_condition_display,
                      extract_reference, parse_table_th_td};
use crate::scrapers::WatchScraper;
use crate::utils::http::fetch_with_retry;

pub struct GrimmeissenScraper {
    config: Arc<Config>,
}

impl GrimmeissenScraper {
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
impl WatchScraper for GrimmeissenScraper {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>> {
        let site_config = self.site_config();
        info!("Scraping Grimmeissen...");
        
        let response = fetch_with_retry(client, &site_config.url, 3).await?;
        let html = response.text().await?;
        
        // Extract all data synchronously
        let watch_data = extract_watch_data(&html, &site_config.base_url)?;
        
        info!("Found {} watch items on Grimmeissen listing page", watch_data.len());
        
        let mut listings = Vec::new();
        
        // Process each watch with async operations
        for data in watch_data {
            if !data.url.is_empty() {
                match self.process_watch(data, client, site_config).await {
                    Ok(listing) => listings.push(listing),
                    Err(e) => error!("Error parsing Grimmeissen item: {}", e),
                }
            }
        }
        
        Ok(listings)
    }
    
    fn site_config(&self) -> &SiteConfig {
        &self.config.sites["grimmeissen"]
    }
    
    fn site_key(&self) -> Site {
        Site::Grimmeissen
    }
}

fn extract_watch_data(html: &str, base_url: &str) -> Result<Vec<WatchData>> {
    let document = Html::parse_document(html);
    let watch_selector = Selector::parse("article.watch")
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
        if let Ok(link_selector) = Selector::parse("figure a") {
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
        if let Ok(img_selector) = Selector::parse("figure a img") {
            if let Some(img) = element.select(&img_selector).next() {
                if let Some(src) = img.value().attr("data-src").or_else(|| img.value().attr("src")) {
                    if let Ok(base) = Url::parse(base_url) {
                        if let Ok(full_url) = base.join(src) {
                            data.image_url = full_url.to_string();
                        }
                    }
                }
            }
        }
        
        // Extract title and brand
        if let Ok(title_selector) = Selector::parse("section.fh h1") {
            if let Some(title_elem) = element.select(&title_selector).next() {
                data.title = clean_text(&title_elem.text().collect::<String>());
                
                // Extract brand from span a within title
                if let Ok(brand_selector) = Selector::parse("span a") {
                    if let Some(brand_elem) = title_elem.select(&brand_selector).next() {
                        data.brand = clean_text(&brand_elem.text().collect::<String>());
                        let model = data.title.replace(&data.brand, "").trim().to_string();
                        data.model = clean_text(&model);
                    }
                }
            }
        }
        
        // Extract price
        if let Ok(price_selector) = Selector::parse("section.fh p") {
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

impl GrimmeissenScraper {
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
        
        // Fetch detail page for additional information
        info!("Fetching details for Grimmeissen item (URL: {})", data.url);
        
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
    
    // Get detailed title from div.c-7.do-lefty h1.lowpad-b
    if let Ok(title_selector) = Selector::parse("div.c-7.do-lefty h1.lowpad-b") {
        if let Some(title_elem) = document.select(&title_selector).next() {
            watch.title = clean_text(&title_elem.text().collect::<String>());
            
            // Re-extract brand from span a
            if let Ok(brand_selector) = Selector::parse("span a") {
                if let Some(brand_elem) = title_elem.select(&brand_selector).next() {
                    watch.brand = clean_text(&brand_elem.text().collect::<String>());
                    let model = watch.title.replace(&watch.brand, "").trim().to_string();
                    watch.model = clean_text(&model);
                }
            }
        }
    }
    
    // Get details container
    if let Ok(details_selector) = Selector::parse("div.c-7.do-lefty") {
        if let Some(details_container) = document.select(&details_selector).next() {
            // Parse first table
            if let Ok(table1_selector) = Selector::parse("table:nth-of-type(1)") {
                if let Some(table1) = details_container.select(&table1_selector).next() {
                    let table_html = table1.html();
                    let headers_map = std::collections::HashMap::from([
                        ("Referenz", "reference"),
                        ("Zustand", "condition_text_raw"),
                        ("Gehäuse", "case_material"),
                        ("Jahr", "year_text"),
                        ("Durchmesser", "diameter"),
                    ]);
                    
                    let table1_data = parse_table_th_td(&table_html, &headers_map);
                    
                    if let Some(ref_val) = table1_data.get("reference") {
                        watch.reference = extract_reference(ref_val);
                    }
                    
                    if let Some(condition) = table1_data.get("condition_text_raw") {
                        watch.condition_display = get_condition_display(condition, Site::Grimmeissen, None);
                    }
                    
                    if let Some(material) = table1_data.get("case_material") {
                        watch.case_material = clean_text(material);
                    }
                    
                    if let Some(year) = table1_data.get("year_text") {
                        watch.year = parse_year_from_string(year, Some(&watch.title));
                    }
                    
                    if let Some(diameter) = table1_data.get("diameter") {
                        watch.diameter = clean_text(diameter);
                    }
                }
            }
            
            // Look for Details section and Lieferumfang
            if let Ok(h3_selector) = Selector::parse("h3") {
                for h3 in details_container.select(&h3_selector) {
                    let h3_text = clean_text(&h3.text().collect::<String>());
                    if h3_text.to_lowercase().contains("details") {
                        // Try to find the next sibling table
                        let mut next = h3.next_sibling();
                        while let Some(node) = next {
                            if let Some(elem) = scraper::ElementRef::wrap(node) {
                                if elem.value().name() == "table" {
                                    let table_html = elem.html();
                                    let headers_map = std::collections::HashMap::from([
                                        ("Lieferumfang", "lieferumfang_text"),
                                    ]);
                                    
                                    let table2_data = parse_table_th_td(&table_html, &headers_map);
                                    
                                    if let Some(lieferumfang) = table2_data.get("lieferumfang_text") {
                                        let (papers, box_status) = parse_box_papers_status(lieferumfang);
                                        watch.papers_status = papers;
                                        watch.box_status = box_status;
                                    }
                                    break;
                                }
                            }
                            next = node.next_sibling();
                        }
                    }
                }
            }
        }
    }
}