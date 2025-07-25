pub mod condition;
pub mod details;
pub mod price;

pub use condition::*;
pub use details::*;
pub use price::*;

use html_escape::decode_html_entities;
use crate::models::EMOJI_QUESTION;

/// Clean and normalize text by removing extra whitespace and decoding HTML entities
pub fn clean_text(text: &str) -> String {
    let decoded = decode_html_entities(text);
    decoded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Parse a table with th/td structure into a HashMap
use std::collections::HashMap;
use scraper::{Html, Selector};

pub fn parse_table_th_td(table_html: &str, headers_map: &HashMap<&str, &str>) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let fragment = Html::parse_fragment(table_html);
    
    let row_selector = Selector::parse("tr").unwrap();
    let th_selector = Selector::parse("th").unwrap();
    let td_selector = Selector::parse("td").unwrap();
    
    for row in fragment.select(&row_selector) {
        if let Some(th) = row.select(&th_selector).next() {
            let th_text = clean_text(&th.text().collect::<String>());
            
            if let Some(&field_name) = headers_map.get(th_text.as_str()) {
                if let Some(td) = row.select(&td_selector).next() {
                    let td_text = clean_text(&td.text().collect::<String>());
                    result.insert(field_name.to_string(), td_text);
                }
            }
        }
    }
    
    result
}