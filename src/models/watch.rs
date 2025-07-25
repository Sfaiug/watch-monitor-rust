use super::EMOJI_QUESTION;
use serde::{Deserialize, Serialize};
use std::fmt;

// NewType pattern for type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WatchId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference(pub String);

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxStatus {
    Yes,
    No,
    Unknown,
}

impl fmt::Display for BoxStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoxStatus::Yes => write!(f, "✅"),
            BoxStatus::No => write!(f, "❌"),
            BoxStatus::Unknown => write!(f, "❓"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PapersStatus {
    Yes,
    No,
    Unknown,
}

impl fmt::Display for PapersStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PapersStatus::Yes => write!(f, "✅"),
            PapersStatus::No => write!(f, "❌"),
            PapersStatus::Unknown => write!(f, "❓"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    Excellent,
    VeryGood,
    Good,
    Fair,
    Unknown,
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Condition::Excellent => write!(f, "Excellent"),
            Condition::VeryGood => write!(f, "Very Good"),
            Condition::Good => write!(f, "Good"),
            Condition::Fair => write!(f, "Fair"),
            Condition::Unknown => write!(f, "❓"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchListing {
    pub brand: String,
    pub model: String,
    pub reference: String,
    pub year: String,
    pub price_eur_display: String,
    pub price_eur_raw_for_hash: String,
    pub price_usd_raw_for_hash: Option<String>, // For TropicalWatch
    pub papers_status: PapersStatus,
    pub box_status: BoxStatus,
    pub condition_display: String,
    pub case_material: String,
    pub diameter: String,
    pub title: String,
    pub watch_url: String,
    pub image_url: String,
    pub site_name: String,
}

impl Default for WatchListing {
    fn default() -> Self {
        Self {
            brand: EMOJI_QUESTION.to_string(),
            model: EMOJI_QUESTION.to_string(),
            reference: EMOJI_QUESTION.to_string(),
            year: EMOJI_QUESTION.to_string(),
            price_eur_display: EMOJI_QUESTION.to_string(),
            price_eur_raw_for_hash: String::new(),
            price_usd_raw_for_hash: None,
            papers_status: PapersStatus::Unknown,
            box_status: BoxStatus::Unknown,
            condition_display: EMOJI_QUESTION.to_string(),
            case_material: EMOJI_QUESTION.to_string(),
            diameter: EMOJI_QUESTION.to_string(),
            title: EMOJI_QUESTION.to_string(),
            watch_url: String::new(),
            image_url: String::new(),
            site_name: String::new(),
        }
    }
}

impl WatchListing {
    pub fn generate_composite_id(&self) -> WatchId {
        use md5::Context;
        
        let brand_norm = self.brand.to_lowercase().trim().to_string();
        let model_norm = self.model.to_lowercase().trim().to_string();
        let ref_norm = self.reference.to_lowercase().replace(' ', "");
        let year_norm = self.year.to_lowercase().trim().to_string();
        
        let price_for_hash = if let Some(usd_price) = &self.price_usd_raw_for_hash {
            usd_price.clone()
        } else {
            self.price_eur_raw_for_hash.clone()
        };
        
        let case_material_norm = if self.case_material != EMOJI_QUESTION {
            self.case_material.to_lowercase().trim().to_string()
        } else {
            String::new()
        };
        
        let components = vec![
            brand_norm.clone(),
            model_norm.clone(),
            ref_norm.clone(),
            price_for_hash.clone(),
            year_norm.clone(),
            case_material_norm.clone(),
        ];
        
        let id_string = components
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("|");
        
        // Check if we have enough essential components
        let essential_count = [&brand_norm, &model_norm, &ref_norm, &price_for_hash]
            .iter()
            .filter(|s| !s.is_empty() && s.as_str() != EMOJI_QUESTION && !s.trim().is_empty())
            .count();
        
        let hash_string = if id_string.is_empty() || id_string.matches('|').count() < 2 || essential_count < 2 {
            // Fallback logic matching Python
            let mut fallback_parts = vec![
                self.title.to_lowercase().trim().to_string(),
                price_for_hash,
            ];
            
            if !self.watch_url.is_empty() {
                fallback_parts.push(self.watch_url.to_lowercase().trim().to_string());
            }
            
            fallback_parts
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("|")
        } else {
            id_string
        };
        
        let mut hasher = Context::new();
        hasher.consume(hash_string.as_bytes());
        let result = hasher.compute();
        
        WatchId(format!("{:x}", result))
    }
}