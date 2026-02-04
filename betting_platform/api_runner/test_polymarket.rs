use std::env;

mod integration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    // Create client
    let client = integration::polymarket_public::PolymarketPublicClient::new()?;
    
    // Test fetch
    match client.get_markets(5).await {
        Ok(markets) => {
            println!("Successfully fetched {} markets", markets.len());
            for market in &markets {
                println!("Market: {}", market.question);
            }
        }
        Err(e) => {
            println!("Error fetching markets: {}", e);
        }
    }
    
    Ok(())
}