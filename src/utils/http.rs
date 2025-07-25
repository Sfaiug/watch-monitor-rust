use anyhow::{Context, Result};
use reqwest::{Client, ClientBuilder, Response};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, warn};

pub fn create_client() -> Result<Client> {
    let client = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(25))
        .pool_max_idle_per_host(6)
        .build()?;
    
    Ok(client)
}

pub async fn fetch_with_retry(client: &Client, url: &str, max_retries: u32) -> Result<Response> {
    let mut attempts = 0;
    let mut last_error = None;
    
    while attempts < max_retries {
        match client.get(url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    return Ok(response);
                } else {
                    let status = response.status();
                    warn!("HTTP error {}: {}", status, url);
                    last_error = Some(anyhow::anyhow!("HTTP error: {}", status));
                }
            }
            Err(e) => {
                error!("Request failed for {}: {}", url, e);
                last_error = Some(e.into());
            }
        }
        
        attempts += 1;
        if attempts < max_retries {
            let delay = Duration::from_secs(2u64.pow(attempts));
            warn!("Retrying in {:?}... (attempt {}/{})", delay, attempts + 1, max_retries);
            sleep(delay).await;
        }
    }
    
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Max retries exceeded")))
        .context(format!("Failed to fetch {} after {} attempts", url, max_retries))
}

