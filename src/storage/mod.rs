use async_trait::async_trait;
use anyhow::Result;
use crate::models::{Site, WatchId};

mod sqlite;
pub use sqlite::SqliteStorage;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn migrate(&self) -> Result<()>;
    async fn has_seen(&self, site: &Site, watch_id: &WatchId) -> Result<bool>;
    async fn mark_seen(&self, site: &Site, watch_id: &WatchId) -> Result<()>;
    async fn import_from_json(&self, json_path: &str) -> Result<()>;
}