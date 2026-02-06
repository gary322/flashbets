//! Simple integration tests for the API server

#[cfg(test)]
mod tests {
    use serde_json::json;

    fn live_base_url() -> Option<String> {
        let enabled = std::env::var("FLASHBETS_LIVE_TESTS")
            .ok()
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);

        if !enabled {
            return None;
        }

        Some(std::env::var("FLASHBETS_LIVE_BASE_URL").unwrap_or_else(|_| "http://localhost:8081".to_string()))
    }
    
    #[tokio::test]
    async fn test_server_health() {
        let Some(base_url) = live_base_url() else {
            return;
        };

        // Try to connect to local server
        let client = reqwest::Client::new();
        match client.get(format!("{}/health", base_url)).send().await {
            Ok(response) => {
                assert_eq!(response.status(), 200);
                let body: serde_json::Value = response.json().await.unwrap();
                assert_eq!(body["status"], "ok");
            }
            Err(_) => {
                println!("Server not running, skipping integration test");
            }
        }
    }
    
    #[tokio::test]
    async fn test_get_markets() {
        let Some(base_url) = live_base_url() else {
            return;
        };

        let client = reqwest::Client::new();
        match client.get(format!("{}/api/markets?limit=1", base_url)).send().await {
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
        let Some(base_url) = live_base_url() else {
            return;
        };

        let client = reqwest::Client::new();
        let auth_request = json!({
            "initial_balance": 10000
        });
        
        match client
            .post(format!("{}/api/demo/create", base_url))
            .json(&auth_request)
            .send()
            .await
        {
            Ok(response) => {
                assert_eq!(response.status(), 200);
                let body: serde_json::Value = response.json().await.unwrap();
                assert!(body["wallet_address"].is_string());
            }
            Err(_) => {
                println!("Server not running, skipping integration test");
            }
        }
    }
}
