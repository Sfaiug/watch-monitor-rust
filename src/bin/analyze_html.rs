use anyhow::Result;
use reqwest::Client;
use scraper::{Html, Selector};
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36")
        .build()?;
    
    // Analyze World of Time
    println!("Fetching World of Time HTML...");
    let response = client
        .get("https://www.worldoftime.de/Watches/NewArrivals")
        .send()
        .await?;
    let html = response.text().await?;
    fs::write("worldoftime_sample.html", &html)?;
    
    let document = Html::parse_document(&html);
    
    // Look for product listings
    let product_selector = Selector::parse("div.product-item, article.product, div.watch-item, li.product").unwrap();
    let products = document.select(&product_selector);
    println!("Found {} potential product elements", products.count());
    
    // Try common selectors
    let selectors = vec![
        "div.product-grid-item",
        "div.product-item-info",
        "article.watch",
        "div.watch-listing",
        "li.watch",
    ];
    
    for selector_str in selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            let count = document.select(&selector).count();
            if count > 0 {
                println!("Selector '{}' matched {} elements", selector_str, count);
            }
        }
    }
    
    // Analyze Grimmeissen
    println!("\nFetching Grimmeissen HTML...");
    let response = client
        .get("https://www.grimmeissen.de/de/uhren")
        .send()
        .await?;
    let html = response.text().await?;
    fs::write("grimmeissen_sample.html", &html)?;
    
    let document = Html::parse_document(&html);
    let article_selector = Selector::parse("article.watch").unwrap();
    let articles = document.select(&article_selector);
    println!("Found {} article.watch elements", articles.count());
    
    Ok(())
}