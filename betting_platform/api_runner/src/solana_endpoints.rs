//! Solana-specific endpoints for RPC health, transaction status, and blockchain operations

use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use tracing::{info, warn, error};

use crate::{
    AppState,
    jwt_validation::AuthenticatedUser,
    solana_rpc_service::{SolanaRpcService, HealthStatus},
    solana_transaction_manager::{SolanaTransactionManager, TransactionManagerStatus},
};

/// Get Solana RPC health status
pub async fn get_rpc_health(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let health = rpc_service.get_health_status().await;
    
    Ok(Json(health))
}

/// Get transaction manager status
pub async fn get_transaction_manager_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let tx_manager = state.solana_tx_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let status = tx_manager.get_status().await;
    
    Ok(Json(status))
}

/// Transaction status query parameters
#[derive(Debug, Deserialize)]
pub struct TransactionStatusQuery {
    pub signature: String,
    pub wait_for_confirmation: Option<bool>,
    pub timeout_ms: Option<u64>,
}

/// Enhanced transaction status response
#[derive(Debug, Serialize)]
pub struct TransactionStatusResponse {
    pub signature: String,
    pub status: String,
    pub confirmed: bool,
    pub slot: Option<u64>,
    pub confirmations: Option<usize>,
    pub err: Option<String>,
    pub block_time: Option<i64>,
    pub compute_units_consumed: Option<u64>,
}

/// Get transaction status with optional waiting
pub async fn get_transaction_status_enhanced(
    State(state): State<AppState>,
    Query(params): Query<TransactionStatusQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let signature = Signature::from_str(&params.signature)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let wait_for_confirmation = params.wait_for_confirmation.unwrap_or(false);
    let timeout_ms = params.timeout_ms.unwrap_or(30000);
    
    let start = std::time::Instant::now();
    
    loop {
        match rpc_service.get_transaction_status(&signature).await {
            Ok(Some(status)) => {
                let response = TransactionStatusResponse {
                    signature: signature.to_string(),
                    status: if status.confirmed { "confirmed" } else { "pending" }.to_string(),
                    confirmed: status.confirmed,
                    slot: status.slot,
                    confirmations: status.confirmations,
                    err: status.err,
                    block_time: None, // Would need additional RPC call
                    compute_units_consumed: None, // Would need additional RPC call
                };
                
                if status.confirmed || !wait_for_confirmation {
                    return Ok(Json(response));
                }
            }
            Ok(None) => {
                if !wait_for_confirmation {
                    let response = TransactionStatusResponse {
                        signature: signature.to_string(),
                        status: "not_found".to_string(),
                        confirmed: false,
                        slot: None,
                        confirmations: None,
                        err: None,
                        block_time: None,
                        compute_units_consumed: None,
                    };
                    return Ok(Json(response));
                }
            }
            Err(e) => {
                error!("Failed to get transaction status: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        
        if start.elapsed().as_millis() > timeout_ms as u128 {
            let response = TransactionStatusResponse {
                signature: signature.to_string(),
                status: "timeout".to_string(),
                confirmed: false,
                slot: None,
                confirmations: None,
                err: Some("Timeout waiting for confirmation".to_string()),
                block_time: None,
                compute_units_consumed: None,
            };
            return Ok(Json(response));
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

/// Account balance response
#[derive(Debug, Serialize)]
pub struct AccountBalanceResponse {
    pub address: String,
    pub lamports: u64,
    pub sol: f64,
    pub executable: bool,
    pub owner: String,
    pub rent_epoch: u64,
}

/// Get account balance and information
pub async fn get_account_info(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let pubkey = Pubkey::from_str(&address)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match rpc_service.get_account(&pubkey).await {
        Ok(Some(account)) => {
            let response = AccountBalanceResponse {
                address,
                lamports: account.lamports,
                sol: account.lamports as f64 / 1_000_000_000.0,
                executable: account.executable,
                owner: account.owner.to_string(),
                rent_epoch: account.rent_epoch,
            };
            Ok(Json(response))
        }
        Ok(None) => {
            let response = AccountBalanceResponse {
                address,
                lamports: 0,
                sol: 0.0,
                executable: false,
                owner: "11111111111111111111111111111111".to_string(),
                rent_epoch: 0,
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get account info: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Batch account query request
#[derive(Debug, Deserialize)]
pub struct BatchAccountQuery {
    pub addresses: Vec<String>,
}

/// Batch account response
#[derive(Debug, Serialize)]
pub struct BatchAccountResponse {
    pub accounts: Vec<Option<AccountBalanceResponse>>,
}

/// Get multiple accounts in batch
pub async fn get_multiple_accounts(
    State(state): State<AppState>,
    Json(query): Json<BatchAccountQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Parse pubkeys
    let pubkeys: Result<Vec<Pubkey>, _> = query.addresses
        .iter()
        .map(|addr| Pubkey::from_str(addr))
        .collect();
    
    let pubkeys = pubkeys.map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match rpc_service.get_multiple_accounts(&pubkeys).await {
        Ok(accounts) => {
            let responses: Vec<Option<AccountBalanceResponse>> = accounts
                .into_iter()
                .zip(query.addresses.iter())
                .map(|(account_opt, address)| {
                    account_opt.map(|account| AccountBalanceResponse {
                        address: address.clone(),
                        lamports: account.lamports,
                        sol: account.lamports as f64 / 1_000_000_000.0,
                        executable: account.executable,
                        owner: account.owner.to_string(),
                        rent_epoch: account.rent_epoch,
                    })
                })
                .collect();
            
            Ok(Json(BatchAccountResponse { accounts: responses }))
        }
        Err(e) => {
            error!("Failed to get multiple accounts: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Recent blockhash response
#[derive(Debug, Serialize)]
pub struct RecentBlockhashResponse {
    pub blockhash: String,
    pub last_valid_block_height: u64,
}

/// Get recent blockhash for transaction building
pub async fn get_recent_blockhash(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    match rpc_service.get_latest_blockhash().await {
        Ok(blockhash) => {
            // Note: We don't have block height in this implementation
            // In production, you'd want to use get_latest_blockhash_with_commitment
            let response = RecentBlockhashResponse {
                blockhash: blockhash.to_string(),
                last_valid_block_height: 0, // Would need additional RPC call
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get recent blockhash: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Simulate transaction request
#[derive(Debug, Deserialize)]
pub struct SimulateTransactionRequest {
    pub transaction: String, // Base64 encoded transaction
    pub sig_verify: Option<bool>,
    pub accounts: Option<Vec<String>>,
}

/// Simulate transaction response
#[derive(Debug, Serialize)]
pub struct SimulateTransactionResponse {
    pub success: bool,
    pub error: Option<String>,
    pub logs: Vec<String>,
    pub units_consumed: Option<u64>,
}

/// Simulate transaction before sending
pub async fn simulate_transaction(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Json(request): Json<SimulateTransactionRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Decode transaction
    let tx_bytes = base64::decode(&request.transaction)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let transaction: Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match rpc_service.simulate_transaction(&transaction).await {
        Ok(()) => {
            let response = SimulateTransactionResponse {
                success: true,
                error: None,
                logs: vec!["Simulation successful".to_string()],
                units_consumed: None,
            };
            Ok(Json(response))
        }
        Err(e) => {
            let response = SimulateTransactionResponse {
                success: false,
                error: Some(e.to_string()),
                logs: vec![],
                units_consumed: None,
            };
            Ok(Json(response))
        }
    }
}

/// Program accounts query
#[derive(Debug, Deserialize)]
pub struct ProgramAccountsQuery {
    pub data_size: Option<usize>,
    pub memcmp_offset: Option<usize>,
    pub memcmp_bytes: Option<String>,
    pub limit: Option<usize>,
}

/// Program account response
#[derive(Debug, Serialize)]
pub struct ProgramAccountResponse {
    pub pubkey: String,
    pub lamports: u64,
    pub owner: String,
    pub data_len: usize,
}

/// Get program accounts with filters
pub async fn get_program_accounts(
    State(state): State<AppState>,
    Path(program_id): Path<String>,
    Query(params): Query<ProgramAccountsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let program_pubkey = Pubkey::from_str(&program_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Build filters
    let mut filters = Vec::new();
    
    if let Some(data_size) = params.data_size {
        filters.push(solana_client::rpc_filter::RpcFilterType::DataSize(data_size as u64));
    }
    
    if let (Some(offset), Some(bytes)) = (params.memcmp_offset, params.memcmp_bytes) {
        let bytes = base64::decode(&bytes)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        
        filters.push(solana_client::rpc_filter::RpcFilterType::Memcmp(
            solana_client::rpc_filter::Memcmp {
                offset,
                bytes: solana_client::rpc_filter::MemcmpEncodedBytes::Base64(
                    base64::encode(&bytes)
                ),
                encoding: None,
            }
        ));
    }
    
    match rpc_service.get_program_accounts(&program_pubkey, filters).await {
        Ok(accounts) => {
            let limit = params.limit.unwrap_or(100);
            let responses: Vec<ProgramAccountResponse> = accounts
                .into_iter()
                .take(limit)
                .map(|(pubkey, account)| ProgramAccountResponse {
                    pubkey: pubkey.to_string(),
                    lamports: account.lamports,
                    owner: account.owner.to_string(),
                    data_len: account.data.len(),
                })
                .collect();
            
            Ok(Json(responses))
        }
        Err(e) => {
            error!("Failed to get program accounts: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Airdrop request for testnet/devnet
#[derive(Debug, Deserialize)]
pub struct AirdropRequest {
    pub address: String,
    pub lamports: Option<u64>,
}

/// Airdrop response
#[derive(Debug, Serialize)]
pub struct AirdropResponse {
    pub signature: String,
    pub lamports: u64,
    pub message: String,
}

/// Request airdrop (testnet/devnet only)
pub async fn request_airdrop(
    State(state): State<AppState>,
    Json(request): Json<AirdropRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check if we're on testnet/devnet
    let rpc_service = state.solana_rpc_service.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let pubkey = Pubkey::from_str(&request.address)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let lamports = request.lamports.unwrap_or(1_000_000_000); // Default 1 SOL
    
    // Note: This is a placeholder - actual airdrop would use RPC client's request_airdrop
    let response = AirdropResponse {
        signature: "Airdrop not available on this network".to_string(),
        lamports: 0,
        message: "Airdrop is only available on testnet/devnet".to_string(),
    };
    
    warn!("Airdrop requested for {} but not available", request.address);
    
    Ok(Json(response))
}