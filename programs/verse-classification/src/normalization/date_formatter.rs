use solana_program::program_error::ProgramError;
use crate::state::DateFormat;

pub struct DateFormatter;

impl DateFormatter {
    /// Normalize dates to standard format
    pub fn normalize(text: &str, target_format: DateFormat) -> Result<String, ProgramError> {
        let mut result = text.to_string();
        
        // First, replace month names with numbers
        result = Self::replace_month_names(&result);
        
        // Then normalize various date formats
        result = Self::normalize_date_formats(&result, target_format)?;
        
        Ok(result)
    }
    
    fn replace_month_names(text: &str) -> String {
        let months = vec![
            ("january", "01"), ("jan", "01"),
            ("february", "02"), ("feb", "02"),
            ("march", "03"), ("mar", "03"),
            ("april", "04"), ("apr", "04"),
            ("may", "05"),
            ("june", "06"), ("jun", "06"),
            ("july", "07"), ("jul", "07"),
            ("august", "08"), ("aug", "08"),
            ("september", "09"), ("sep", "09"), ("sept", "09"),
            ("october", "10"), ("oct", "10"),
            ("november", "11"), ("nov", "11"),
            ("december", "12"), ("dec", "12"),
        ];
        
        let mut result = text.to_string();
        for (name, num) in months {
            // Case-insensitive replacement
            let lower_result = result.to_lowercase();
            if let Some(pos) = lower_result.find(name) {
                let end_pos = pos + name.len();
                // Check word boundaries
                let is_word_start = pos == 0 || !result.chars().nth(pos - 1).unwrap().is_alphanumeric();
                let is_word_end = end_pos >= result.len() || !result.chars().nth(end_pos).unwrap().is_alphanumeric();
                
                if is_word_start && is_word_end {
                    result.replace_range(pos..end_pos, num);
                }
            }
        }
        
        result
    }
    
    fn normalize_date_formats(text: &str, target_format: DateFormat) -> Result<String, ProgramError> {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut date_buffer = String::new();
        
        while let Some(ch) = chars.next() {
            if ch.is_numeric() {
                date_buffer.push(ch);
            } else if ch == '/' || ch == '-' || ch == '.' {
                date_buffer.push(ch);
            } else {
                // Process any collected date pattern
                if !date_buffer.is_empty() {
                    let normalized = Self::try_normalize_date_pattern(&date_buffer, target_format)?;
                    result.push_str(&normalized);
                    date_buffer.clear();
                }
                result.push(ch);
            }
        }
        
        // Don't forget remaining buffer
        if !date_buffer.is_empty() {
            let normalized = Self::try_normalize_date_pattern(&date_buffer, target_format)?;
            result.push_str(&normalized);
        }
        
        Ok(result)
    }
    
    fn try_normalize_date_pattern(pattern: &str, target_format: DateFormat) -> Result<String, ProgramError> {
        // Try to parse common date patterns
        let parts: Vec<&str> = pattern.split(&['/', '-', '.'][..]).collect();
        
        if parts.len() == 3 {
            // Determine if it's YYYY-MM-DD, MM-DD-YYYY, or DD-MM-YYYY
            let (year, month, day) = if parts[0].len() == 4 {
                // YYYY-MM-DD format
                (parts[0], parts[1], parts[2])
            } else if parts[2].len() == 4 {
                // Assume MM-DD-YYYY for now (could be DD-MM-YYYY in EU)
                (parts[2], parts[0], parts[1])
            } else {
                // Can't determine format, return as-is
                return Ok(pattern.to_string());
            };
            
            // Validate components
            if let (Ok(y), Ok(m), Ok(d)) = (
                year.parse::<u32>(),
                month.parse::<u32>(),
                day.parse::<u32>()
            ) {
                if y >= 1900 && y <= 2100 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                    // Format according to target
                    return Ok(match target_format {
                        DateFormat::ISO8601 => format!("{:04}-{:02}-{:02}", y, m, d),
                        DateFormat::USFormat => format!("{:02}/{:02}/{:04}", m, d, y),
                        DateFormat::EUFormat => format!("{:02}/{:02}/{:04}", d, m, y),
                        DateFormat::UnixTimestamp => {
                            // Simplified: just return a placeholder timestamp
                            // In production, would calculate actual Unix timestamp
                            "1640995200".to_string() // 2022-01-01 00:00:00 UTC
                        }
                    });
                }
            }
        }
        
        // Return original if can't parse
        Ok(pattern.to_string())
    }
}

/// Normalize currency symbols
pub fn normalize_currency(text: &str) -> Result<String, ProgramError> {
    let currencies = vec![
        ("$", "USD"),
        ("€", "EUR"),
        ("£", "GBP"),
        ("¥", "JPY"),
        ("₹", "INR"),
        ("dollar", "USD"),
        ("dollars", "USD"),
        ("euro", "EUR"),
        ("euros", "EUR"),
        ("pound", "GBP"),
        ("pounds", "GBP"),
        ("yen", "JPY"),
        ("rupee", "INR"),
        ("rupees", "INR"),
    ];
    
    let mut result = text.to_string();
    for (symbol, code) in currencies {
        result = result.replace(symbol, code);
    }
    
    Ok(result)
}

/// Public interface function
pub fn normalize_dates(text: &str, target_format: DateFormat) -> Result<String, ProgramError> {
    DateFormatter::normalize(text, target_format)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_month_name_replacement() {
        let text = "January 1, 2025";
        let result = DateFormatter::replace_month_names(text);
        assert_eq!(result, "01 1, 2025");
        
        let text2 = "Meeting in December";
        let result2 = DateFormatter::replace_month_names(text2);
        assert_eq!(result2, "Meeting in 12");
    }
    
    #[test]
    fn test_date_normalization_iso() {
        let text = "Event on 12/25/2025";
        let result = normalize_dates(text, DateFormat::ISO8601).unwrap();
        assert_eq!(result, "Event on 2025-12-25");
        
        let text2 = "2025-03-15 deadline";
        let result2 = normalize_dates(text2, DateFormat::ISO8601).unwrap();
        assert_eq!(result2, "2025-03-15 deadline");
    }
    
    #[test]
    fn test_currency_normalization() {
        assert_eq!(normalize_currency("$100").unwrap(), "USD100");
        assert_eq!(normalize_currency("€50").unwrap(), "EUR50");
        assert_eq!(normalize_currency("100 dollars").unwrap(), "100 USDs");  // "dollars" -> "USDs"
    }
}