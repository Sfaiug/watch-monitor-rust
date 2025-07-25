use regex::Regex;
use once_cell::sync::Lazy;
use crate::models::{BoxStatus, PapersStatus, EMOJI_QUESTION};

static YEAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(19[4-9]\d|20[0-2]\d)")
        .expect("Invalid year regex")
});

/// Parse year from text string, optionally checking title as well
pub fn parse_year_from_string(text: &str, title: Option<&str>) -> String {
    // First try the main text
    if let Some(captures) = YEAR_REGEX.find(text) {
        return captures.as_str().to_string();
    }
    
    // Then try the title if provided
    if let Some(title_str) = title {
        if let Some(captures) = YEAR_REGEX.find(title_str) {
            return captures.as_str().to_string();
        }
    }
    
    EMOJI_QUESTION.to_string()
}

/// Parse box and papers status from text
pub fn parse_box_papers_status(text: &str) -> (PapersStatus, BoxStatus) {
    let text_lower = text.to_lowercase();
    
    // Check for explicit no box/papers
    let no_box = text_lower.contains("ohne box") || 
                 text_lower.contains("no box") ||
                 text_lower.contains("kein box");
    
    let no_papers = text_lower.contains("ohne papiere") || 
                    text_lower.contains("no papers") ||
                    text_lower.contains("keine papiere") ||
                    text_lower.contains("no certificate");
    
    // Check for presence of box/papers
    let has_box = text_lower.contains("mit box") ||
                  text_lower.contains("with box") ||
                  text_lower.contains("originalbox") ||
                  text_lower.contains("original box") ||
                  (text_lower.contains("box") && !no_box);
    
    let has_papers = text_lower.contains("mit papiere") ||
                     text_lower.contains("with papers") ||
                     text_lower.contains("certificate") ||
                     text_lower.contains("garantie") ||
                     text_lower.contains("warranty") ||
                     (text_lower.contains("papier") && !no_papers);
    
    let papers = if no_papers {
        PapersStatus::No
    } else if has_papers {
        PapersStatus::Yes
    } else {
        PapersStatus::Unknown
    };
    
    let box_status = if no_box {
        BoxStatus::No
    } else if has_box {
        BoxStatus::Yes
    } else {
        BoxStatus::Unknown
    };
    
    (papers, box_status)
}

/// Extract reference number from various formats
pub fn extract_reference(text: &str) -> String {
    let text = super::clean_text(text);
    
    // Remove common prefixes
    let cleaned = text
        .replace("Ref.", "")
        .replace("Ref", "")
        .replace("Reference", "")
        .replace(":", "")
        .trim()
        .to_string();
    
    if cleaned.is_empty() {
        EMOJI_QUESTION.to_string()
    } else {
        cleaned
    }
}