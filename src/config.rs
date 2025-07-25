use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sites: HashMap<String, SiteConfig>,
    pub check_interval_seconds: u64,
    pub user_agent: String,
    pub exchange_rate_api_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub url: String,
    pub webhook: String,
    pub name: String,
    pub color: u32,
    pub base_url: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        // For now, hardcode the configuration matching the Python script
        let mut sites = HashMap::new();
        
        sites.insert(
            "worldoftime".to_string(),
            SiteConfig {
                url: "https://www.worldoftime.de/Watches/NewArrivals".to_string(),
                webhook: "https://discord.com/api/webhooks/1356956538190823534/GMUibI4sDu9I515zDvxyC0cqkFiXC_D4yh89L36WsRIdIzSlTmtFx4LTtxxsodYBSqXB".to_string(),
                name: "World of Time".to_string(),
                color: 0x2F4F4F,
                base_url: "https://www.worldoftime.de".to_string(),
            },
        );
        
        sites.insert(
            "grimmeissen".to_string(),
            SiteConfig {
                url: "https://www.grimmeissen.de/de/uhren".to_string(),
                webhook: "https://discord.com/api/webhooks/1353748268584009759/AmGqjGwQyzkexl6p9WSQY0JfmIsLcnEAjnxNEE4OUva-3F5ZNNWzcFj5lB7gXG4kw-I_".to_string(),
                name: "Grimmeissen".to_string(),
                color: 0xDAA520,
                base_url: "https://www.grimmeissen.de".to_string(),
            },
        );
        
        sites.insert(
            "tropicalwatch".to_string(),
            SiteConfig {
                url: "https://tropicalwatch.com/?sort=recent".to_string(),
                webhook: "https://discord.com/api/webhooks/1356956912163225700/oTbe-SP7V1zgtccFWrNFD4p5vw4uzSPyJ8D9nhQKcb9c9ZkKfImV7ZDQwrFCuxMy07wd".to_string(),
                name: "Tropical Watch".to_string(),
                color: 0x008080,
                base_url: "https://tropicalwatch.com".to_string(),
            },
        );
        
        sites.insert(
            "juwelier_exchange".to_string(),
            SiteConfig {
                url: "https://www.juwelier-exchange.de/uhren".to_string(),
                webhook: "https://discord.com/api/webhooks/1376895131432784014/h_1ML2z1qtLTQ_SuU7YqF9l8xOF2BdB1LoAecQVvvUPO2ejojZB6H_8RnatL7c82Ew3p".to_string(),
                name: "Juwelier Exchange".to_string(),
                color: 0xB08D57,
                base_url: "https://www.juwelier-exchange.de".to_string(),
            },
        );
        
        sites.insert(
            "watch_out".to_string(),
            SiteConfig {
                url: "https://www.watch-out.shop/collections/gebrauchte-uhren?sort_by=created-descending".to_string(),
                webhook: "https://discord.com/api/webhooks/1376895816312291348/Hhhf6asQRoKlPzf5E_NYz0fA7VsSUphPDeBLWyLGcHw324qEorsH6B7bH8gdhzcc6SOi".to_string(),
                name: "Watch Out".to_string(),
                color: 0xC0C0C0,
                base_url: "https://www.watch-out.shop".to_string(),
            },
        );
        
        sites.insert(
            "rueschenbeck".to_string(),
            SiteConfig {
                url: "https://www.rueschenbeck.de/vintage-certified-pre-owned".to_string(),
                webhook: "https://discord.com/api/webhooks/1376895941533110333/XwN3ZJcRqnrAE_LE9LO4KIEekPnkwGw-ibpxJQ8F9BmNYbfErhBSHhQ7fmSOFDaYXmGw".to_string(),
                name: "Rüschenbeck".to_string(),
                color: 0xCFB53B,
                base_url: "https://www.rueschenbeck.de".to_string(),
            },
        );

        Ok(Config {
            sites,
            check_interval_seconds: 60,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36".to_string(),
            exchange_rate_api_url: "https://api.exchangerate-api.com/v4/latest/USD".to_string(),
        })
    }
}