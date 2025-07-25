use async_trait::async_trait;
use anyhow::Result;
use reqwest::Client;
use crate::config::SiteConfig;
use crate::models::{Site, WatchListing};

mod worldoftime;
mod grimmeissen;
mod tropicalwatch;
mod juwelier_exchange;
mod watch_out;
mod rueschenbeck;

pub use worldoftime::WorldOfTimeScraper;
pub use grimmeissen::GrimmeissenScraper;
pub use tropicalwatch::TropicalWatchScraper;
pub use juwelier_exchange::JuwelierExchangeScraper;
pub use watch_out::WatchOutScraper;
pub use rueschenbeck::RueschenbeckScraper;

#[async_trait]
pub trait WatchScraper: Send + Sync {
    async fn scrape(&self, client: &Client) -> Result<Vec<WatchListing>>;
    fn site_config(&self) -> &SiteConfig;
    fn site_key(&self) -> Site;
}