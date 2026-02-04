use solana_program::program_error::ProgramError;

pub struct NumberStandardizer;

impl NumberStandardizer {
    /// Standardize numbers in text (e.g., "1k" -> "1000", "1M" -> "1000000")
    pub fn standardize(text: &str) -> Result<String, ProgramError> {
        let mut result = text.to_string();
        
        // Handle common abbreviations with regex-like patterns
        let patterns = vec![
            ("k", 1_000),
            ("K", 1_000),
            ("m", 1_000_000),
            ("M", 1_000_000),
            ("b", 1_000_000_000),
            ("B", 1_000_000_000),
            ("t", 1_000_000_000_000),
            ("T", 1_000_000_000_000),
        ];
        
        for (suffix, multiplier) in patterns {
            result = Self::replace_number_suffix(&result, suffix, multiplier)?;
        }
        
        // Remove commas from numbers
        result = result.replace(',', "");
        
        // Convert written numbers to digits
        result = Self::convert_written_numbers(&result);
        
        Ok(result)
    }
    
    fn replace_number_suffix(text: &str, suffix: &str, multiplier: u64) -> Result<String, ProgramError> {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut number_buffer = String::new();
        
        while let Some(ch) = chars.next() {
            if ch.is_numeric() || ch == '.' {
                number_buffer.push(ch);
            } else if !number_buffer.is_empty() {
                // Check if this character is our suffix
                if ch.to_string() == suffix {
                    // Check if next char is not alphanumeric (to avoid matching inside words)
                    let is_word_boundary = chars.peek().map_or(true, |&next| {
                        !next.is_alphanumeric()
                    });
                    
                    if is_word_boundary {
                        // Parse the number and multiply
                        if let Ok(num) = number_buffer.parse::<f64>() {
                            let expanded = (num * multiplier as f64) as u64;
                            result.push_str(&expanded.to_string());
                            number_buffer.clear();
                            continue;
                        }
                    }
                }
                // Not our suffix or failed to parse, write original
                result.push_str(&number_buffer);
                number_buffer.clear();
                result.push(ch);
            } else {
                result.push(ch);
            }
        }
        
        // Don't forget remaining buffer
        if !number_buffer.is_empty() {
            result.push_str(&number_buffer);
        }
        
        Ok(result)
    }
    
    fn convert_written_numbers(text: &str) -> String {
        let replacements = vec![
            ("zero", "0"),
            ("one", "1"),
            ("two", "2"),
            ("three", "3"),
            ("four", "4"),
            ("five", "5"),
            ("six", "6"),
            ("seven", "7"),
            ("eight", "8"),
            ("nine", "9"),
            ("ten", "10"),
            ("eleven", "11"),
            ("twelve", "12"),
            ("thirteen", "13"),
            ("fourteen", "14"),
            ("fifteen", "15"),
            ("sixteen", "16"),
            ("seventeen", "17"),
            ("eighteen", "18"),
            ("nineteen", "19"),
            ("twenty", "20"),
            ("thirty", "30"),
            ("forty", "40"),
            ("fifty", "50"),
            ("sixty", "60"),
            ("seventy", "70"),
            ("eighty", "80"),
            ("ninety", "90"),
            ("hundred", "100"),
            ("thousand", "1000"),
            ("million", "1000000"),
            ("billion", "1000000000"),
            ("trillion", "1000000000000"),
        ];
        
        let mut result = text.to_string();
        for (word, number) in replacements {
            // Use word boundaries to avoid partial replacements
            let pattern = format!(" {} ", word);
            let replacement = format!(" {} ", number);
            result = result.replace(&pattern, &replacement);
            
            // Also handle at start and end of string
            if result.starts_with(&format!("{} ", word)) {
                result = result.replacen(word, number, 1);
            }
            if result.ends_with(&format!(" {}", word)) {
                let pos = result.rfind(word).unwrap();
                result.replace_range(pos..pos + word.len(), number);
            }
        }
        
        result
    }
}

// Public interface function
pub fn standardize_numbers(text: &str) -> Result<String, ProgramError> {
    NumberStandardizer::standardize(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_number_abbreviations() {
        assert_eq!(standardize_numbers("150k").unwrap(), "150000");
        assert_eq!(standardize_numbers("1.5M").unwrap(), "1500000");
        assert_eq!(standardize_numbers("2.5B").unwrap(), "2500000000");
        assert_eq!(standardize_numbers("3T").unwrap(), "3000000000000");
    }
    
    #[test]
    fn test_comma_removal() {
        assert_eq!(standardize_numbers("1,000,000").unwrap(), "1000000");
        assert_eq!(standardize_numbers("150,000").unwrap(), "150000");
    }
    
    #[test]
    fn test_written_numbers() {
        assert_eq!(standardize_numbers("twenty thousand").unwrap(), "20 1000");
        assert_eq!(standardize_numbers("one million").unwrap(), "1 1000000");
        assert_eq!(standardize_numbers("five hundred").unwrap(), "5 100");
    }
    
    #[test]
    fn test_mixed_content() {
        assert_eq!(
            standardize_numbers("BTC above 150k by December").unwrap(),
            "BTC above 150000 by December"
        );
        assert_eq!(
            standardize_numbers("market cap of $2.5B").unwrap(),
            "market cap of $2500000000"
        );
    }
}