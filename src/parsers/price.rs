use once_cell::sync::Lazy;
use regex::Regex;
use crate::models::EMOJI_QUESTION;

static PRICE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d{1,3}(?:[.,]\d{3})*(?:[.,]\d{2})?)")
        .expect("Invalid price regex")
});

/// Extract price string for hashing (normalized format)
pub fn get_price_string_for_hash(price_text: &str) -> String {
    if let Some(captures) = PRICE_REGEX.find(price_text) {
        let price_str = captures.as_str();
        // Normalize to use dots for thousands and comma for decimal
        price_str
            .replace('.', "")
            .replace(',', ".")
            .trim()
            .to_string()
    } else {
        String::new()
    }
}

/// Format price for display with EUR symbol
pub fn format_price_eur_display(price_text: &str) -> String {
    if price_text.trim().is_empty() {
        return EMOJI_QUESTION.to_string();
    }
    
    // Extract numeric part
    if let Some(captures) = PRICE_REGEX.find(price_text) {
        let price_str = captures.as_str();
        
        // Check if already has EUR symbol
        if price_text.contains('€') || price_text.contains("EUR") {
            // Keep original formatting but ensure EUR symbol
            if price_text.contains('€') {
                price_text.to_string()
            } else {
                format!("{} €", price_str)
            }
        } else {
            // Add EUR symbol
            format!("{} €", price_str)
        }
    } else {
        EMOJI_QUESTION.to_string()
    }
}

/// Convert USD price to EUR and format for display
pub fn convert_usd_to_eur_display(usd_price: f64, exchange_rate: f64) -> String {
    let eur_price = usd_price * exchange_rate;
    
    // Format with thousands separator
    let formatted = if eur_price >= 1000.0 {
        let thousands = (eur_price / 1000.0) as i32;
        let remainder = (eur_price % 1000.0) as i32;
        if remainder == 0 {
            format!("{}.000", thousands)
        } else {
            format!("{}.{:03}", thousands, remainder)
        }
    } else {
        format!("{:.0}", eur_price)
    };
    
    format!("{} €", formatted)
}

/// Parse USD price from text
pub fn parse_usd_price(price_text: &str) -> Option<f64> {
    // Remove currency symbols and clean
    let cleaned = price_text
        .replace('$', "")
        .replace("USD", "")
        .replace(',', "")
        .trim()
        .to_string();
    
    cleaned.parse::<f64>().ok()
}