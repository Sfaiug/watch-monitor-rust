use chrono::Local;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::{json, Value};

use crate::config::SiteConfig;
use crate::models::{
    WatchListing, EMOJI_BOX, EMOJI_CONDITION, EMOJI_DIAMETER, EMOJI_MATERIAL,
    EMOJI_PAPERS, EMOJI_PRICE, EMOJI_QUESTION, EMOJI_REFERENCE, EMOJI_SEARCH, EMOJI_YEAR,
};
use crate::parsers::clean_text;

pub fn create_embed(listing: &WatchListing, site_config: &SiteConfig) -> Value {
    // Build embed title matching Python logic
    let embed_title = build_embed_title(listing);
    
    // Build Chrono24 search link
    let chrono_link = build_chrono24_link(listing);
    
    // Build fields array
    let mut fields = Vec::new();
    
    // Price field (always shown)
    fields.push(json!({
        "name": format!("{} Price:", EMOJI_PRICE),
        "value": format!("**{}**", listing.price_eur_display),
        "inline": false
    }));
    
    // Reference field (only if not in title and not ❓)
    let reference_clean = listing.reference.replace(EMOJI_QUESTION, "");
    if listing.reference != EMOJI_QUESTION && !embed_title.contains(&reference_clean) {
        fields.push(json!({
            "name": format!("{} Reference:", EMOJI_REFERENCE),
            "value": format!("**{}**", listing.reference),
            "inline": false
        }));
    }
    
    // Chrono24 search link
    fields.push(json!({
        "name": format!("{} Chrono24 Search:", EMOJI_SEARCH),
        "value": format!("[**Search similar**]({})", chrono_link),
        "inline": false
    }));
    
    // Bottom fields (only shown if not ❓)
    let mut bottom_fields = Vec::new();
    
    if listing.year != EMOJI_QUESTION {
        bottom_fields.push(json!({
            "name": format!("{} Year:", EMOJI_YEAR),
            "value": format!("**{}**", listing.year),
            "inline": true
        }));
    }
    
    if listing.condition_display != EMOJI_QUESTION {
        bottom_fields.push(json!({
            "name": format!("{} Condition:", EMOJI_CONDITION),
            "value": format!("**{}**", listing.condition_display),
            "inline": true
        }));
    }
    
    let box_str = listing.box_status.to_string();
    if box_str != EMOJI_QUESTION {
        bottom_fields.push(json!({
            "name": format!("{} Box:", EMOJI_BOX),
            "value": format!("**{}**", box_str),
            "inline": true
        }));
    }
    
    let papers_str = listing.papers_status.to_string();
    if papers_str != EMOJI_QUESTION {
        bottom_fields.push(json!({
            "name": format!("{} Papers:", EMOJI_PAPERS),
            "value": format!("**{}**", papers_str),
            "inline": true
        }));
    }
    
    if listing.case_material != EMOJI_QUESTION {
        bottom_fields.push(json!({
            "name": format!("{} Case Material:", EMOJI_MATERIAL),
            "value": format!("**{}**", listing.case_material),
            "inline": true
        }));
    }
    
    if listing.diameter != EMOJI_QUESTION {
        bottom_fields.push(json!({
            "name": format!("{} Diameter:", EMOJI_DIAMETER),
            "value": format!("**{}**", listing.diameter),
            "inline": true
        }));
    }
    
    // Add spacing and bottom fields if any exist
    if !bottom_fields.is_empty() {
        fields.push(json!({
            "name": "\u{200B}",
            "value": "\u{200B}",
            "inline": false
        }));
        fields.extend(bottom_fields);
    }
    
    // Build the embed
    json!({
        "title": embed_title,
        "url": listing.watch_url,
        "color": site_config.color,
        "image": {
            "url": listing.image_url
        },
        "fields": fields,
        "footer": {
            "text": format!("{} - Detected: {}", 
                site_config.name,
                Local::now().format("%Y-%m-%d %H:%M:%S")
            )
        }
    })
}

fn build_embed_title(listing: &WatchListing) -> String {
    let brand = clean_text(&listing.brand);
    let model = clean_text(&listing.model);
    let reference = clean_text(&listing.reference).replace(EMOJI_QUESTION, "");
    let full_title = clean_text(&listing.title);
    
    let mut title_parts = Vec::new();
    
    // Add brand if not ❓
    if !brand.is_empty() && brand != EMOJI_QUESTION {
        title_parts.push(brand.clone());
    }
    
    // Add model if distinct from brand
    if !model.is_empty() && model != EMOJI_QUESTION && model.to_lowercase() != brand.to_lowercase() {
        let mut model_cleaned = model.clone();
        
        // Avoid "Brand Brand Model" pattern
        if !brand.is_empty() && model_cleaned.to_lowercase().starts_with(&brand.to_lowercase()) {
            model_cleaned = model_cleaned[brand.len()..].trim().to_string();
        }
        
        if !model_cleaned.is_empty() {
            title_parts.push(model_cleaned);
        }
    }
    
    let mut embed_title = title_parts.join(" ");
    
    // If title is just brand or empty, try to use full title
    if embed_title.is_empty() || embed_title.to_lowercase() == brand.to_lowercase() {
        if !full_title.is_empty() {
            let mut temp_title = full_title.clone();
            
            // Remove brand from beginning if present
            if !brand.is_empty() && temp_title.to_lowercase().starts_with(&brand.to_lowercase()) {
                temp_title = temp_title[brand.len()..].trim().to_string();
            }
            
            // Remove common prefixes
            temp_title = regex::Regex::new(r"^(Herrenuhr|Damenuhr|Unisexuhr)\s*")
                .unwrap()
                .replace(&temp_title, "")
                .trim()
                .to_string();
            
            // Remove reference if present
            if !reference.is_empty() {
                temp_title = temp_title.replace(&reference, "").trim().to_string();
            }
            
            // Clean up common trailing descriptors
            temp_title = regex::Regex::new(r"\s*(Automatik|Quarz|Chrono|GMT|Date|Certified Pre-Owned|Stahl|Gold|Keramik)$")
                .unwrap()
                .replace(&temp_title, "")
                .trim()
                .to_string();
            
            // Clean up quotes
            temp_title = temp_title.replace("''", "'").trim_matches('\'').trim().to_string();
            
            if !temp_title.is_empty() && temp_title.to_lowercase() != brand.to_lowercase() {
                embed_title = if !brand.is_empty() {
                    format!("{} {}", brand, temp_title)
                } else {
                    temp_title
                };
            } else if !full_title.is_empty() {
                embed_title = full_title;
            }
        } else {
            embed_title = if !brand.is_empty() { brand } else { "N/A Watch".to_string() };
        }
    }
    
    // Append reference if not already in title
    if !reference.is_empty() && !embed_title.contains(&reference) {
        embed_title = format!("{} | {}", embed_title, reference);
    }
    
    // Truncate if too long
    if embed_title.len() > 250 {
        embed_title = format!("{}...", &embed_title[..250]);
    } else if embed_title.trim().is_empty() || embed_title == EMOJI_QUESTION {
        embed_title = "Unknown Watch".to_string();
    }
    
    embed_title
}

fn build_chrono24_link(listing: &WatchListing) -> String {
    let brand = if listing.brand != EMOJI_QUESTION { &listing.brand } else { "" };
    let model = if listing.model != EMOJI_QUESTION && 
                   listing.model.to_lowercase() != brand.to_lowercase() { 
        &listing.model 
    } else { 
        "" 
    };
    let reference = listing.reference.replace(EMOJI_QUESTION, "");
    
    let mut query_parts = Vec::new();
    
    if !brand.is_empty() {
        query_parts.push(brand.to_string());
    }
    
    if !model.is_empty() {
        query_parts.push(model.to_string());
    }
    
    if !reference.is_empty() {
        query_parts.push(reference);
    }
    
    let query = query_parts.join(" ");
    let encoded_query = utf8_percent_encode(&query, NON_ALPHANUMERIC).to_string();
    
    format!("https://www.chrono24.de/search/index.htm?dosearch=true&query={}&sortorder=1", encoded_query)
}