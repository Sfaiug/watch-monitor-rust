use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};

#[derive(Debug, Clone)]
pub struct ExchangeRateCache {
    rate: Option<f64>,
    last_updated: Option<DateTime<Utc>>,
}

impl Default for ExchangeRateCache {
    fn default() -> Self {
        Self {
            rate: None,
            last_updated: None,
        }
    }
}


pub struct ExchangeRateClient {
    cache: Arc<Mutex<ExchangeRateCache>>,
}

impl ExchangeRateClient {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(ExchangeRateCache::default())),
        }
    }
    
    pub async fn get_usd_to_eur_rate(&self, client: &Client) -> Result<f64> {
        let mut cache = self.cache.lock().await;
        
        // Check if cache is valid (less than 24 hours old)
        if let (Some(rate), Some(last_updated)) = (cache.rate, cache.last_updated) {
            if Utc::now() - last_updated < Duration::hours(24) {
                info!("Using cached USD to EUR rate: {}", rate);
                return Ok(rate);
            }
        }
        
        // Fetch new rate
        info!("Fetching fresh USD to EUR exchange rate");
        
        // Using exchangerate-api.com free tier
        let url = "https://api.exchangerate-api.com/v4/latest/USD";
        
        match client.get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await 
        {
            Ok(response) => {
                if response.status().is_success() {
                    let data: serde_json::Value = response.json().await?;
                    
                    if let Some(rates) = data.get("rates") {
                        if let Some(eur_rate) = rates.get("EUR") {
                            if let Some(rate_value) = eur_rate.as_f64() {
                                info!("Successfully fetched USD to EUR rate: {}", rate_value);
                                cache.rate = Some(rate_value);
                                cache.last_updated = Some(Utc::now());
                                return Ok(rate_value);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch exchange rate: {}", e);
            }
        }
        
        // Fallback to cached rate if available, or use a default
        if let Some(rate) = cache.rate {
            info!("Using stale cached rate due to fetch failure: {}", rate);
            Ok(rate)
        } else {
            // Default fallback rate
            let fallback_rate = 0.92;
            info!("Using fallback USD to EUR rate: {}", fallback_rate);
            cache.rate = Some(fallback_rate);
            cache.last_updated = Some(Utc::now());
            Ok(fallback_rate)
        }
    }
}