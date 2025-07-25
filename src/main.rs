use anyhow::Result;
use chrono::Local;
use futures::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

mod config;
mod discord;
mod models;
mod parsers;
mod scrapers;
mod storage;
mod utils;

use crate::config::Config;
use crate::scrapers::{
    GrimmeissenScraper, JuwelierExchangeScraper, RueschenbeckScraper, TropicalWatchScraper,
    WatchOutScraper, WatchScraper, WorldOfTimeScraper,
};
use crate::storage::{SqliteStorage, Storage};
use crate::utils::exchange_rate::ExchangeRateClient;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("watch_monitor=info".parse()?),
        )
        .init();

    info!("Starting Watch Monitor");

    // Load configuration
    let config = Arc::new(Config::load()?);

    // Initialize storage
    let storage = Arc::new(SqliteStorage::new("watch_monitor.db").await?);
    storage.migrate().await?;

    // Initialize HTTP client with connection pooling
    let client = Arc::new(utils::http::create_client()?);
    
    // Initialize exchange rate client for TropicalWatch
    let exchange_rate_client = Arc::new(ExchangeRateClient::new());

    // Initialize scrapers
    let scrapers: Vec<Box<dyn WatchScraper>> = vec![
        Box::new(WorldOfTimeScraper::new(config.clone())),
        Box::new(GrimmeissenScraper::new(config.clone())),
        Box::new(TropicalWatchScraper::new(config.clone(), exchange_rate_client)),
        Box::new(JuwelierExchangeScraper::new(config.clone())),
        Box::new(WatchOutScraper::new(config.clone())),
        Box::new(RueschenbeckScraper::new(config.clone())),
    ];

    // Main monitoring loop
    let mut interval = interval(Duration::from_secs(config.check_interval_seconds));
    
    loop {
        interval.tick().await;
        
        info!("--- Starting new check cycle at {} ---", Local::now().format("%Y-%m-%d %H:%M:%S"));
        
        // Scrape all sites concurrently
        let scraping_futures = scrapers.iter().map(|scraper| {
            let client = client.clone();
            let storage = storage.clone();
            
            async move {
                let site_name = scraper.site_config().name.clone();
                info!("Processing site: {}", site_name.to_uppercase());
                
                match scraper.scrape(&client).await {
                    Ok(listings) => {
                        info!("Found {} watch items on {}", listings.len(), site_name);
                        
                        let mut new_items = 0;
                        for listing in listings {
                            let watch_id = listing.generate_composite_id();
                            
                            // Check if we've seen this watch before
                            if !storage.has_seen(&scraper.site_key(), &watch_id).await? {
                                // Send Discord notification
                                if let Err(e) = discord::send_notification(
                                    &scraper.site_config().webhook,
                                    &listing,
                                    &scraper.site_config(),
                                ).await {
                                    error!("Failed to send Discord notification: {}", e);
                                }
                                
                                // Mark as seen
                                storage.mark_seen(&scraper.site_key(), &watch_id).await?;
                                new_items += 1;
                                
                                // Small delay between notifications
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        }
                        
                        if new_items == 0 {
                            info!("No new items found on {}", site_name);
                        } else {
                            info!("Found {} new items on {}", new_items, site_name);
                        }
                        
                        Ok::<(), anyhow::Error>(())
                    }
                    Err(e) => {
                        error!("CRITICAL UNHANDLED ERROR in {} scraper: {}", site_name, e);
                        Ok(())
                    }
                }
            }
        });
        
        // Execute all scrapers concurrently
        let results = join_all(scraping_futures).await;
        
        // Log any errors
        for result in results {
            if let Err(e) = result {
                error!("Error in scraping task: {}", e);
            }
        }
        
        info!("Check cycle completed, waiting {} seconds", config.check_interval_seconds);
    }
}