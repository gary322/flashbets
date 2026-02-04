pub mod text_normalizer;
pub mod number_standardizer;
pub mod date_formatter;

pub use text_normalizer::*;
pub use number_standardizer::*;
pub use date_formatter::*;

use crate::state::SynonymGroup;

// Common stopwords for keyword extraction
pub const STOPWORDS: &[&str] = &[
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "i",
    "it", "for", "not", "on", "with", "he", "as", "you", "do", "at",
    "this", "but", "his", "by", "from", "they", "we", "say", "her", "she",
    "or", "an", "will", "my", "one", "all", "would", "there", "their",
    "what", "so", "up", "out", "if", "about", "who", "get", "which", "go",
    "me", "when", "make", "can", "like", "time", "no", "just", "him", "know",
    "take", "people", "into", "year", "your", "good", "some", "could", "them",
    "see", "other", "than", "then", "now", "look", "only", "come", "its", "over",
    "think", "also", "back", "after", "use", "two", "how", "our", "work",
    "first", "well", "way", "even", "new", "want", "because", "any", "these",
    "give", "day", "most", "us", "is", "was", "are", "been", "has", "had",
    "were", "said", "did", "getting", "made", "find", "where", "much", "too",
    "very", "still", "being", "going", "why", "before", "never", "here", "more",
    "between", "under", "such", "through", "same", "above", "below", "each",
    "few", "those", "always", "both", "another", "while", "upon", "every",
    "during", "without", "within", "across", "against", "among", "throughout",
    "toward", "towards", "via", "versus", "whether", "yet", "nor", "per",
];

// Common synonyms for normalization
pub fn get_default_synonyms() -> Vec<SynonymGroup> {
    vec![
        SynonymGroup {
            primary: "bitcoin".to_string(),
            synonyms: vec!["btc".to_string(), "xbt".to_string()],
        },
        SynonymGroup {
            primary: "ethereum".to_string(),
            synonyms: vec!["eth".to_string(), "ether".to_string()],
        },
        SynonymGroup {
            primary: "greater".to_string(),
            synonyms: vec!["above".to_string(), ">".to_string()],
        },
        SynonymGroup {
            primary: "less".to_string(),
            synonyms: vec!["below".to_string(), "<".to_string()],
        },
        SynonymGroup {
            primary: "dollar".to_string(),
            synonyms: vec!["usd".to_string(), "$".to_string()],
        },
    ]
}