use crate::models::{Site, EMOJI_QUESTION};

/// Get condition display string based on raw condition text and site
pub fn get_condition_display(condition_raw: &str, site: Site, description_parts: Option<&[String]>) -> String {
    let condition_lower = condition_raw.to_lowercase();
    
    // Site-specific condition mappings
    match site {
        Site::WorldOfTime => {
            if condition_lower.contains("neu") || condition_lower.contains("new") {
                "New".to_string()
            } else if condition_lower.contains("sehr gut") || condition_lower.contains("very good") {
                "Very Good".to_string()
            } else if condition_lower.contains("gut") || condition_lower.contains("good") {
                "Good".to_string()
            } else if condition_raw != EMOJI_QUESTION {
                condition_raw.to_string()
            } else {
                EMOJI_QUESTION.to_string()
            }
        }
        Site::Grimmeissen => {
            if condition_lower.contains("neuwertig") || condition_lower.contains("like new") {
                "Like New".to_string()
            } else if condition_lower.contains("sehr gut") || condition_lower.contains("very good") {
                "Very Good".to_string()
            } else if condition_lower.contains("gut") || condition_lower.contains("good") {
                "Good".to_string()
            } else if condition_raw != EMOJI_QUESTION {
                condition_raw.to_string()
            } else {
                EMOJI_QUESTION.to_string()
            }
        }
        Site::TropicalWatch => {
            // TropicalWatch uses description to determine condition
            if let Some(parts) = description_parts {
                let desc_text = parts.join(" ").to_lowercase();
                if desc_text.contains("mint") || desc_text.contains("pristine") {
                    "Mint".to_string()
                } else if desc_text.contains("excellent") {
                    "Excellent".to_string()
                } else if desc_text.contains("very good") {
                    "Very Good".to_string()
                } else if desc_text.contains("good") {
                    "Good".to_string()
                } else {
                    EMOJI_QUESTION.to_string()
                }
            } else {
                EMOJI_QUESTION.to_string()
            }
        }
        _ => {
            // Generic condition mapping for other sites
            if condition_raw != EMOJI_QUESTION {
                condition_raw.to_string()
            } else {
                EMOJI_QUESTION.to_string()
            }
        }
    }
}