use solana_program::program_error::ProgramError;

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryRule {
    pub name: String,
    pub keywords: Vec<&'static str>,
    pub patterns: Vec<&'static str>,
    pub min_score: f32,
}

/// Detect category from normalized title and keywords
pub fn detect_category(
    normalized_title: &str,
    keywords: &[String],
) -> Result<String, ProgramError> {
    let category_rules = get_category_rules();
    let mut category_scores = Vec::new();
    
    for rule in &category_rules {
        let score = calculate_category_score(normalized_title, keywords, rule);
        if score >= rule.min_score {
            category_scores.push((rule.name.clone(), score));
        }
    }
    
    // Return highest scoring category, or "general" if none match
    if let Some((category, _)) = category_scores.iter().max_by(|a, b| {
        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
    }) {
        Ok(category.clone())
    } else {
        Ok("general".to_string())
    }
}

fn calculate_category_score(
    normalized_title: &str,
    keywords: &[String],
    rule: &CategoryRule,
) -> f32 {
    let mut score = 0.0;
    
    // Score based on keyword matches
    for keyword in keywords {
        if rule.keywords.contains(&keyword.as_str()) {
            score += 1.0 / rule.keywords.len() as f32;
        }
    }
    
    // Score based on pattern matches in title
    for pattern in &rule.patterns {
        if normalized_title.contains(pattern) {
            score += 0.5;
        }
    }
    
    score
}

fn get_category_rules() -> Vec<CategoryRule> {
    vec![
        // Crypto category
        CategoryRule {
            name: "crypto".to_string(),
            keywords: vec![
                "btc", "bitcoin", "eth", "ethereum", "crypto", "cryptocurrency", 
                "defi", "token", "blockchain", "mining", "wallet", "exchange",
                "stablecoin", "altcoin", "hodl", "satoshi", "wei", "gwei"
            ],
            patterns: vec![
                "bitcoin", "btc", "ethereum", "eth", "crypto",
                "blockchain", "defi", "nft", "web3"
            ],
            min_score: 0.3,
        },
        // Politics/Elections
        CategoryRule {
            name: "politics".to_string(),
            keywords: vec![
                "election", "president", "vote", "democrat", "republican", 
                "trump", "biden", "congress", "senate", "governor", "mayor",
                "campaign", "poll", "primary", "candidate", "party", "political"
            ],
            patterns: vec![
                "election", "president", "vote", "democrat", "republican",
                "congress", "senate", "campaign", "political"
            ],
            min_score: 0.4,
        },
        // Economics
        CategoryRule {
            name: "economics".to_string(),
            keywords: vec![
                "fed", "rate", "inflation", "gdp", "recession", "economy", 
                "unemployment", "market", "stock", "bond", "treasury", "cpi",
                "fomc", "interest", "growth", "deficit", "trade", "tariff"
            ],
            patterns: vec![
                "fed", "federal reserve", "inflation", "recession", "gdp",
                "interest rate", "economy", "economic", "treasury"
            ],
            min_score: 0.3,
        },
        // Technology
        CategoryRule {
            name: "technology".to_string(),
            keywords: vec![
                "ai", "gpt", "tech", "google", "apple", "microsoft", "launch", 
                "release", "software", "hardware", "computer", "phone", "app",
                "internet", "cloud", "data", "algorithm", "machine learning"
            ],
            patterns: vec![
                "ai", "artificial intelligence", "tech", "technology",
                "software", "hardware", "launch", "release"
            ],
            min_score: 0.3,
        },
        // Sports
        CategoryRule {
            name: "sports".to_string(),
            keywords: vec![
                "nfl", "nba", "game", "win", "championship", "team", "player", 
                "score", "football", "basketball", "baseball", "soccer", "tennis",
                "olympics", "tournament", "league", "season", "playoff", "finals"
            ],
            patterns: vec![
                "nfl", "nba", "mlb", "nhl", "fifa", "olympics",
                "championship", "tournament", "game", "match"
            ],
            min_score: 0.35,
        },
        // Weather/Climate
        CategoryRule {
            name: "climate".to_string(),
            keywords: vec![
                "weather", "temperature", "rain", "snow", "hurricane", "storm",
                "climate", "global warming", "drought", "flood", "tornado",
                "celsius", "fahrenheit", "forecast", "meteorology"
            ],
            patterns: vec![
                "weather", "temperature", "climate", "storm",
                "hurricane", "tornado", "forecast"
            ],
            min_score: 0.3,
        },
        // Entertainment
        CategoryRule {
            name: "entertainment".to_string(),
            keywords: vec![
                "movie", "film", "oscar", "grammy", "music", "album", "concert",
                "actor", "actress", "director", "netflix", "spotify", "youtube",
                "tv", "series", "show", "celebrity", "hollywood"
            ],
            patterns: vec![
                "movie", "film", "oscar", "grammy", "netflix",
                "music", "album", "concert", "hollywood"
            ],
            min_score: 0.3,
        },
    ]
}

/// Get parent category for hierarchical organization
pub fn get_parent_category(category: &str) -> Option<String> {
    match category {
        "bitcoin" | "ethereum" | "altcoin" => Some("crypto".to_string()),
        "nfl" | "nba" | "mlb" | "soccer" => Some("sports".to_string()),
        "movie" | "music" | "tv" => Some("entertainment".to_string()),
        "stock" | "bond" | "forex" => Some("economics".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_crypto_detection() {
        let title = "bitcoin price prediction";
        let keywords = vec!["bitcoin".to_string(), "price".to_string(), "prediction".to_string()];
        
        let category = detect_category(title, &keywords).unwrap();
        assert_eq!(category, "crypto");
    }
    
    #[test]
    fn test_politics_detection() {
        let title = "presidential election results";
        let keywords = vec!["presidential".to_string(), "election".to_string(), "results".to_string()];
        
        let category = detect_category(title, &keywords).unwrap();
        assert_eq!(category, "politics");
    }
    
    #[test]
    fn test_general_fallback() {
        let title = "random market event";
        let keywords = vec!["random".to_string(), "market".to_string(), "event".to_string()];
        
        let category = detect_category(title, &keywords).unwrap();
        assert_eq!(category, "general");
    }
    
    #[test]
    fn test_parent_category() {
        assert_eq!(get_parent_category("bitcoin"), Some("crypto".to_string()));
        assert_eq!(get_parent_category("nfl"), Some("sports".to_string()));
        assert_eq!(get_parent_category("general"), None);
    }
}