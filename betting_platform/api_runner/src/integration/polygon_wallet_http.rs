//! HTTP-based Polygon wallet integration for Polymarket
//! This implementation uses HTTP APIs instead of ethers library to avoid dependency conflicts

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};

/// Polygon network configuration
#[derive(Debug, Clone)]
pub struct PolygonConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub usdc_address: String,
    pub conditional_tokens_address: String,
    pub ctf_exchange_address: String,
    pub api_key: Option<String>,
}

impl Default for PolygonConfig {
    fn default() -> Self {
        Self {
            rpc_url: std::env::var("POLYGON_RPC_URL")
                .unwrap_or_else(|_| "https://polygon-rpc.com".to_string()),
            chain_id: 137, // Polygon mainnet
            usdc_address: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string(), // USDC on Polygon
            conditional_tokens_address: "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_string(), // Polymarket CTF
            ctf_exchange_address: "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E".to_string(), // Polymarket Exchange
            api_key: std::env::var("POLYGON_API_KEY").ok(),
        }
    }
}

/// HTTP-based Polygon wallet client
pub struct PolygonWalletHttp {
    config: PolygonConfig,
    client: reqwest::Client,
}

impl PolygonWalletHttp {
    /// Create new Polygon wallet HTTP client
    pub fn new(config: PolygonConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        
        Ok(Self {
            config,
            client,
        })
    }
    
    /// Make JSON-RPC call to Polygon node
    async fn rpc_call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });
        
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse()?);
        
        if let Some(api_key) = &self.config.api_key {
            headers.insert("Authorization", format!("Bearer {}", api_key).parse()?);
        }
        
        let response = self.client
            .post(&self.config.rpc_url)
            .headers(headers)
            .json(&payload)
            .send()
            .await?;
        
        let result: JsonRpcResponse = response.json().await?;
        
        if let Some(error) = result.error {
            return Err(anyhow!("RPC error: {} - {}", error.code, error.message));
        }
        
        result.result.ok_or_else(|| anyhow!("No result in RPC response"))
    }
    
    /// Get ETH balance for an address
    pub async fn get_eth_balance(&self, address: &str) -> Result<String> {
        let params = json!([address, "latest"]);
        let result = self.rpc_call("eth_getBalance", params).await?;
        
        // Convert hex to decimal
        let hex_balance = result.as_str()
            .ok_or_else(|| anyhow!("Invalid balance format"))?;
        
        Ok(hex_to_decimal(hex_balance)?)
    }
    
    /// Get USDC balance for an address
    pub async fn get_usdc_balance(&self, address: &str) -> Result<String> {
        // ERC20 balanceOf method signature
        let method_sig = "0x70a08231"; // balanceOf(address)
        let padded_address = format!("{:0>64}", address.trim_start_matches("0x"));
        let data = format!("{}{}", method_sig, padded_address);
        
        let params = json!([{
            "to": self.config.usdc_address,
            "data": data
        }, "latest"]);
        
        let result = self.rpc_call("eth_call", params).await?;
        
        let hex_balance = result.as_str()
            .ok_or_else(|| anyhow!("Invalid balance format"))?;
        
        Ok(hex_to_decimal(hex_balance)?)
    }
    
    /// Get conditional token balance
    pub async fn get_outcome_token_balance(
        &self,
        address: &str,
        token_id: &str,
    ) -> Result<String> {
        // ERC1155 balanceOf method signature
        let method_sig = "0x00fdd58e"; // balanceOf(address,uint256)
        let padded_address = format!("{:0>64}", address.trim_start_matches("0x"));
        let padded_token_id = format!("{:0>64}", token_id.trim_start_matches("0x"));
        let data = format!("{}{}{}", method_sig, padded_address, padded_token_id);
        
        let params = json!([{
            "to": self.config.conditional_tokens_address,
            "data": data
        }, "latest"]);
        
        let result = self.rpc_call("eth_call", params).await?;
        
        let hex_balance = result.as_str()
            .ok_or_else(|| anyhow!("Invalid balance format"))?;
        
        Ok(hex_to_decimal(hex_balance)?)
    }
    
    /// Get current gas price
    pub async fn get_gas_price(&self) -> Result<String> {
        let result = self.rpc_call("eth_gasPrice", json!([])).await?;
        
        let hex_price = result.as_str()
            .ok_or_else(|| anyhow!("Invalid gas price format"))?;
        
        Ok(hex_to_decimal(hex_price)?)
    }
    
    /// Get transaction count (nonce) for an address
    pub async fn get_transaction_count(&self, address: &str) -> Result<u64> {
        let params = json!([address, "latest"]);
        let result = self.rpc_call("eth_getTransactionCount", params).await?;
        
        let hex_count = result.as_str()
            .ok_or_else(|| anyhow!("Invalid transaction count format"))?;
        
        let count = u64::from_str_radix(hex_count.trim_start_matches("0x"), 16)?;
        Ok(count)
    }
    
    /// Get transaction receipt
    pub async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<TransactionReceipt> {
        let params = json!([tx_hash]);
        let result = self.rpc_call("eth_getTransactionReceipt", params).await?;
        
        serde_json::from_value(result)
            .map_err(|e| anyhow!("Failed to parse transaction receipt: {}", e))
    }
    
    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, tx: &TransactionRequest) -> Result<String> {
        let params = json!([tx]);
        let result = self.rpc_call("eth_estimateGas", params).await?;
        
        let hex_gas = result.as_str()
            .ok_or_else(|| anyhow!("Invalid gas estimate format"))?;
        
        Ok(hex_to_decimal(hex_gas)?)
    }
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

/// Transaction request
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub from: String,
    pub to: String,
    pub value: Option<String>,
    pub data: Option<String>,
    pub gas: Option<String>,
    pub gas_price: Option<String>,
    pub nonce: Option<String>,
}

/// Transaction receipt
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceipt {
    pub transaction_hash: String,
    pub transaction_index: String,
    pub block_hash: String,
    pub block_number: String,
    pub from: String,
    pub to: Option<String>,
    pub cumulative_gas_used: String,
    pub gas_used: String,
    pub contract_address: Option<String>,
    pub logs: Vec<LogEntry>,
    pub status: String,
}

/// Log entry in transaction receipt
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: String,
    pub transaction_hash: String,
    pub transaction_index: String,
    pub block_hash: String,
    pub log_index: String,
    pub removed: bool,
}

/// Wallet info for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletInfo {
    pub address: String,
    pub eth_balance: String,
    pub usdc_balance: String,
    pub chain_id: u64,
    pub chain_name: String,
}

/// Helper function to convert hex string to decimal string
fn hex_to_decimal(hex: &str) -> Result<String> {
    let hex = hex.trim_start_matches("0x");
    if hex.is_empty() || hex == "0" {
        return Ok("0".to_string());
    }
    
    // Parse as u128 for large numbers
    let value = u128::from_str_radix(hex, 16)?;
    Ok(value.to_string())
}

/// Helper function to convert decimal string to hex string
pub fn decimal_to_hex(decimal: &str) -> Result<String> {
    let value: u128 = decimal.parse()?;
    Ok(format!("0x{:x}", value))
}

/// Helper function to pad hex string to 32 bytes
pub fn pad_hex_to_32_bytes(hex: &str) -> String {
    let hex = hex.trim_start_matches("0x");
    format!("0x{:0>64}", hex)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hex_conversion() {
        assert_eq!(hex_to_decimal("0x0").unwrap(), "0");
        assert_eq!(hex_to_decimal("0x1").unwrap(), "1");
        assert_eq!(hex_to_decimal("0xff").unwrap(), "255");
        assert_eq!(hex_to_decimal("0x3e8").unwrap(), "1000");
        
        assert_eq!(decimal_to_hex("0").unwrap(), "0x0");
        assert_eq!(decimal_to_hex("1").unwrap(), "0x1");
        assert_eq!(decimal_to_hex("255").unwrap(), "0xff");
        assert_eq!(decimal_to_hex("1000").unwrap(), "0x3e8");
    }
    
    #[test]
    fn test_hex_padding() {
        assert_eq!(
            pad_hex_to_32_bytes("0x1"),
            "0x0000000000000000000000000000000000000000000000000000000000000001"
        );
        assert_eq!(
            pad_hex_to_32_bytes("0xabc123"),
            "0x0000000000000000000000000000000000000000000000000000000000abc123"
        );
    }
}

// Re-export commonly used json! macro
use serde_json::json;