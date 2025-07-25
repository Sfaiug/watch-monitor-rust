pub mod embed;

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use tracing::{error, info};

use crate::config::SiteConfig;
use crate::models::WatchListing;
use embed::create_embed;

pub async fn send_notification(
    webhook_url: &str,
    listing: &WatchListing,
    site_config: &SiteConfig,
) -> Result<()> {
    let embed = create_embed(listing, site_config);
    
    let payload = json!({
        "embeds": [embed]
    });
    
    let client = Client::new();
    let response = client
        .post(webhook_url)
        .json(&payload)
        .send()
        .await
        .context("Failed to send Discord webhook")?;
    
    if response.status().is_success() {
        info!("Successfully sent Discord notification for {}", listing.title);
        Ok(())
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Discord webhook failed with status {}: {}", status, error_text);
        Err(anyhow::anyhow!("Discord webhook failed: {} - {}", status, error_text))
    }
}