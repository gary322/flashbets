//! Polygon wallet handlers using HTTP-based implementation

use axum::{
    extract::{State, Path, Query},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, error, debug};

use crate::{
    AppState,
    response::responses,
    integration::polygon_wallet_http::{PolygonWalletHttp, PolygonConfig, WalletInfo},
};

/// Get wallet balance
pub async fn get_wallet_balance(
    Path(address): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate address format
    if !address.starts_with("0x") || address.len() != 42 {
        return responses::bad_request("Invalid Ethereum address format").into_response();
    }
    
    // Create wallet HTTP client
    let config = PolygonConfig::default();
    let wallet_client = match PolygonWalletHttp::new(config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create wallet client: {}", e);
            return responses::service_unavailable(&format!("Failed to initialize wallet client: {}", e)).into_response();
        }
    };
    
    // Get balances
    let eth_balance = match wallet_client.get_eth_balance(&address).await {
        Ok(balance) => balance,
        Err(e) => {
            error!("Failed to get ETH balance: {}", e);
            "0".to_string()
        }
    };
    
    let usdc_balance = match wallet_client.get_usdc_balance(&address).await {
        Ok(balance) => balance,
        Err(e) => {
            error!("Failed to get USDC balance: {}", e);
            "0".to_string()
        }
    };
    
    let wallet_info = WalletInfo {
        address: address.clone(),
        eth_balance,
        usdc_balance,
        chain_id: 137,
        chain_name: "Polygon".to_string(),
    };
    
    responses::ok(json!(wallet_info)).into_response()
}

/// Get outcome token balance
#[derive(Deserialize)]
pub struct GetOutcomeBalanceQuery {
    pub token_id: String,
}

pub async fn get_outcome_balance(
    Path(address): Path<String>,
    Query(params): Query<GetOutcomeBalanceQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate address format
    if !address.starts_with("0x") || address.len() != 42 {
        return responses::bad_request("Invalid Ethereum address format").into_response();
    }
    
    // Create wallet HTTP client
    let config = PolygonConfig::default();
    let wallet_client = match PolygonWalletHttp::new(config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create wallet client: {}", e);
            return responses::service_unavailable(&format!("Failed to initialize wallet client: {}", e)).into_response();
        }
    };
    
    // Get outcome token balance
    match wallet_client.get_outcome_token_balance(&address, &params.token_id).await {
        Ok(balance) => {
            responses::ok(json!({
                "address": address,
                "token_id": params.token_id,
                "balance": balance,
                "token_type": "ERC1155",
            })).into_response()
        }
        Err(e) => {
            error!("Failed to get outcome balance: {}", e);
            responses::internal_error(&format!("Failed to get outcome balance: {}", e)).into_response()
        }
    }
}

/// Get gas price
pub async fn get_gas_price(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Create wallet HTTP client
    let config = PolygonConfig::default();
    let wallet_client = match PolygonWalletHttp::new(config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create wallet client: {}", e);
            return responses::service_unavailable(&format!("Failed to initialize wallet client: {}", e)).into_response();
        }
    };
    
    match wallet_client.get_gas_price().await {
        Ok(gas_price) => {
            // Convert wei to gwei
            let gas_price_wei: u128 = gas_price.parse().unwrap_or(0);
            let gas_price_gwei = gas_price_wei / 1_000_000_000;
            
            responses::ok(json!({
                "gas_price_wei": gas_price,
                "gas_price_gwei": gas_price_gwei.to_string(),
                "chain_id": 137,
                "chain_name": "Polygon",
            })).into_response()
        }
        Err(e) => {
            error!("Failed to get gas price: {}", e);
            responses::internal_error(&format!("Failed to get gas price: {}", e)).into_response()
        }
    }
}

/// Get transaction receipt
#[derive(Deserialize)]
pub struct GetTransactionQuery {
    pub tx_hash: String,
}

pub async fn get_transaction_receipt(
    Query(params): Query<GetTransactionQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate transaction hash format
    if !params.tx_hash.starts_with("0x") || params.tx_hash.len() != 66 {
        return responses::bad_request("Invalid transaction hash format").into_response();
    }
    
    // Create wallet HTTP client
    let config = PolygonConfig::default();
    let wallet_client = match PolygonWalletHttp::new(config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create wallet client: {}", e);
            return responses::service_unavailable(&format!("Failed to initialize wallet client: {}", e)).into_response();
        }
    };
    
    match wallet_client.get_transaction_receipt(&params.tx_hash).await {
        Ok(receipt) => {
            responses::ok(json!(receipt)).into_response()
        }
        Err(e) => {
            error!("Failed to get transaction receipt: {}", e);
            responses::internal_error(&format!("Failed to get transaction receipt: {}", e)).into_response()
        }
    }
}

/// Estimate gas for USDC approval
#[derive(Deserialize)]
pub struct EstimateGasRequest {
    pub from: String,
    pub spender: String,
    pub amount: String,
}

pub async fn estimate_gas_approval(
    State(state): State<AppState>,
    Json(payload): Json<EstimateGasRequest>,
) -> impl IntoResponse {
    // Validate addresses
    if !payload.from.starts_with("0x") || payload.from.len() != 42 {
        return responses::bad_request("Invalid from address format").into_response();
    }
    if !payload.spender.starts_with("0x") || payload.spender.len() != 42 {
        return responses::bad_request("Invalid spender address format").into_response();
    }
    
    // Create wallet HTTP client
    let config = PolygonConfig::default();
    let wallet_client = match PolygonWalletHttp::new(config.clone()) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create wallet client: {}", e);
            return responses::service_unavailable(&format!("Failed to initialize wallet client: {}", e)).into_response();
        }
    };
    
    // Create approval transaction data
    let method_sig = "0x095ea7b3"; // approve(address,uint256)
    let padded_spender = format!("{:0>64}", payload.spender.trim_start_matches("0x"));
    let amount_hex = crate::integration::polygon_wallet_http::decimal_to_hex(&payload.amount).unwrap_or("0x0".to_string());
    let padded_amount = format!("{:0>64}", amount_hex.trim_start_matches("0x"));
    let data = format!("{}{}{}", method_sig, padded_spender, padded_amount);
    
    let tx_request = crate::integration::polygon_wallet_http::TransactionRequest {
        from: payload.from.clone(),
        to: config.usdc_address.clone(),
        value: None,
        data: Some(data),
        gas: None,
        gas_price: None,
        nonce: None,
    };
    
    match wallet_client.estimate_gas(&tx_request).await {
        Ok(gas_estimate) => {
            responses::ok(json!({
                "gas_estimate": gas_estimate,
                "from": payload.from,
                "to": config.usdc_address,
                "spender": payload.spender,
                "amount": payload.amount,
            })).into_response()
        }
        Err(e) => {
            error!("Failed to estimate gas: {}", e);
            responses::internal_error(&format!("Failed to estimate gas: {}", e)).into_response()
        }
    }
}

/// Get wallet nonce (transaction count)
pub async fn get_wallet_nonce(
    Path(address): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate address format
    if !address.starts_with("0x") || address.len() != 42 {
        return responses::bad_request("Invalid Ethereum address format").into_response();
    }
    
    // Create wallet HTTP client
    let config = PolygonConfig::default();
    let wallet_client = match PolygonWalletHttp::new(config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create wallet client: {}", e);
            return responses::service_unavailable(&format!("Failed to initialize wallet client: {}", e)).into_response();
        }
    };
    
    match wallet_client.get_transaction_count(&address).await {
        Ok(nonce) => {
            responses::ok(json!({
                "address": address,
                "nonce": nonce,
                "chain_id": 137,
            })).into_response()
        }
        Err(e) => {
            error!("Failed to get nonce: {}", e);
            responses::internal_error(&format!("Failed to get nonce: {}", e)).into_response()
        }
    }
}