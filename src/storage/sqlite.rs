use async_trait::async_trait;
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::models::{Site, WatchId};
use crate::storage::Storage;

pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    pub async fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .context("Failed to open SQLite database")?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS seen_watches (
                site TEXT NOT NULL,
                watch_id TEXT NOT NULL,
                first_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (site, watch_id)
            )",
            [],
        )?;
        
        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_site ON seen_watches(site)",
            [],
        )?;
        
        info!("Database migration completed");
        Ok(())
    }
    
    async fn has_seen(&self, site: &Site, watch_id: &WatchId) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        let count: Option<i32> = conn
            .query_row(
                "SELECT 1 FROM seen_watches WHERE site = ?1 AND watch_id = ?2",
                params![site.key(), &watch_id.0],
                |row| row.get(0),
            )
            .optional()?;
        
        Ok(count.is_some())
    }
    
    async fn mark_seen(&self, site: &Site, watch_id: &WatchId) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT OR IGNORE INTO seen_watches (site, watch_id) VALUES (?1, ?2)",
            params![site.key(), &watch_id.0],
        )?;
        
        Ok(())
    }
    
    async fn import_from_json(&self, json_path: &str) -> Result<()> {
        if !Path::new(json_path).exists() {
            info!("No existing JSON file to import");
            return Ok(());
        }
        
        let content = std::fs::read_to_string(json_path)?;
        let data: serde_json::Value = serde_json::from_str(&content)?;
        
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        
        if let Some(obj) = data.as_object() {
            for (site_key, watch_ids) in obj {
                if let Some(site) = Site::from_key(site_key) {
                    if let Some(ids) = watch_ids.as_array() {
                        for id in ids {
                            if let Some(id_str) = id.as_str() {
                                tx.execute(
                                    "INSERT OR IGNORE INTO seen_watches (site, watch_id) VALUES (?1, ?2)",
                                    params![site.key(), id_str],
                                )?;
                            }
                        }
                    }
                }
            }
        }
        
        tx.commit()?;
        info!("Successfully imported data from {}", json_path);
        Ok(())
    }
}