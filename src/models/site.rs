use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Site {
    WorldOfTime,
    Grimmeissen,
    TropicalWatch,
    JuwelierExchange,
    WatchOut,
    Rueschenbeck,
}

impl Site {
    pub fn key(&self) -> &'static str {
        match self {
            Site::WorldOfTime => "worldoftime",
            Site::Grimmeissen => "grimmeissen",
            Site::TropicalWatch => "tropicalwatch",
            Site::JuwelierExchange => "juwelier_exchange",
            Site::WatchOut => "watch_out",
            Site::Rueschenbeck => "rueschenbeck",
        }
    }
    
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "worldoftime" => Some(Site::WorldOfTime),
            "grimmeissen" => Some(Site::Grimmeissen),
            "tropicalwatch" => Some(Site::TropicalWatch),
            "juwelier_exchange" => Some(Site::JuwelierExchange),
            "watch_out" => Some(Site::WatchOut),
            "rueschenbeck" => Some(Site::Rueschenbeck),
            _ => None,
        }
    }
}