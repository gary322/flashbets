use std::collections::HashMap;

// Simple test to verify verse catalog
fn main() {
    // Simulate the build_verse_catalog function
    let catalog = build_test_catalog();
    println!("Verse catalog size: {}", catalog.len());
    
    // Print first 5 verses
    for (i, (id, verse)) in catalog.iter().enumerate() {
        if i >= 5 { break; }
        println!("Verse {}: {} - {} ({}x)", i+1, id, verse.name, verse.multiplier);
    }
}

#[derive(Debug, Clone)]
struct GeneratedVerse {
    id: String,
    name: String,
    description: String,
    level: u8,
    multiplier: f64,
    category: String,
    risk_tier: String,
    parent_id: Option<String>,
    market_count: u32,
}

fn build_test_catalog() -> HashMap<String, GeneratedVerse> {
    let mut catalog = HashMap::new();
    
    // Add a few test verses
    catalog.insert("verse_cat_politics".to_string(), GeneratedVerse {
        id: "verse_cat_politics".to_string(),
        name: "Political Markets".to_string(),
        description: "Elections, policy, and political outcome markets".to_string(),
        level: 1,
        multiplier: 1.5,
        category: "Politics".to_string(),
        risk_tier: "Low".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    catalog.insert("verse_cat_crypto".to_string(), GeneratedVerse {
        id: "verse_cat_crypto".to_string(),
        name: "Cryptocurrency Markets".to_string(),
        description: "Digital asset prices, adoption, and crypto events".to_string(),
        level: 1,
        multiplier: 1.8,
        category: "Crypto".to_string(),
        risk_tier: "Medium".to_string(),
        parent_id: None,
        market_count: 0,
    });
    
    catalog
}