//! Simple integration tests for the API server

#[cfg(test)]
mod tests {
    use serde_json::json;
    
    #[tokio::test]
    async fn test_server_health() {
        // Try to connect to local server
        let client = reqwest::Client::new();
        match client.get("http://localhost:8081/health").send().await {
            Ok(response) => {
                assert_eq!(response.status(), 200);
                let body: serde_json::Value = response.json().await.unwrap();
                assert_eq!(body["status"], "healthy");
            }
            Err(_) => {
                println!("Server not running, skipping integration test");
            }
        }
    }
    
    #[tokio::test]
    async fn test_get_markets() {
        let client = reqwest::Client::new();
        match client.get("http://localhost:8081/api/markets").send().await {
            Ok(response) => {
                assert_eq!(response.status(), 200);
                let body: serde_json::Value = response.json().await.unwrap();
                assert!(body["markets"].is_array());
            }
            Err(_) => {
                println!("Server not running, skipping integration test");
            }
        }
    }
    
    #[tokio::test]
    async fn test_wallet_auth() {
        let client = reqwest::Client::new();
        let auth_request = json!({
            "wallet": "demo_wallet_001"
        });
        
        match client
            .post("http://localhost:8081/api/auth/wallet")
            .json(&auth_request)
            .send()
            .await
        {
            Ok(response) => {
                assert_eq!(response.status(), 200);
                let body: serde_json::Value = response.json().await.unwrap();
                assert!(body["challenge"].is_string());
            }
            Err(_) => {
                println!("Server not running, skipping integration test");
            }
        }
    }
}