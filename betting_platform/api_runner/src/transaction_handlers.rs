//! Transaction signing and submission handlers
//! 
//! Provides endpoints for preparing unsigned transactions for client-side signing
//! and submitting signed transactions to the Solana network

use axum::{
    extract::{State, Path},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use tracing::{debug, error, info};
use crate::{
    AppState,
    middleware::AuthenticatedUser,
    response::responses,
    transaction_signing::{TransactionBuilder, PreparedTransaction, prepare_transaction as prepare_tx},
    types::{CreateMarketRequest, PlaceTradeRequest, ClosePositionRequest, MarketType},
};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};

/// Transaction preparation request
#[derive(Debug, Deserialize)]
pub struct PrepareTransactionRequest {
    /// Type of transaction to prepare
    pub transaction_type: TransactionType,
    /// Transaction-specific parameters
    pub params: serde_json::Value,
    /// Wallet public key
    pub wallet: String,
}

/// Transaction types
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    CreateMarket,
    PlaceTrade,
    ClosePosition,
    CreateDemoAccount,
}

/// Transaction submission request
#[derive(Debug, Deserialize)]
pub struct SubmitTransactionRequest {
    /// Base64 encoded signed transaction
    pub signed_transaction: String,
    /// Optional wait for confirmation
    #[serde(default = "default_wait_confirmation")]
    pub wait_for_confirmation: bool,
}

fn default_wait_confirmation() -> bool {
    true
}

/// Transaction submission response
#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    /// Transaction signature
    pub signature: String,
    /// Whether transaction was confirmed
    pub confirmed: bool,
    /// Slot number if confirmed
    pub slot: Option<u64>,
    /// Block time if confirmed
    pub block_time: Option<i64>,
    /// Error if transaction failed
    pub error: Option<String>,
}

/// Prepare transaction for client-side signing
pub async fn prepare_transaction(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Json(payload): Json<PrepareTransactionRequest>,
) -> Response {
    let wallet = match Pubkey::from_str(&payload.wallet) {
        Ok(pk) => pk,
        Err(e) => return responses::bad_request(&format!("Invalid wallet: {}", e)).into_response(),
    };
    
    // Get recent blockhash
    let recent_blockhash = match state.rpc_client.get_latest_blockhash() {
        Ok(bh) => bh,
        Err(e) => return responses::service_unavailable(&format!("Failed to get blockhash: {}", e)).into_response(),
    };
    
    // Build transaction based on type
    let transaction = match payload.transaction_type {
        TransactionType::CreateMarket => {
            let params: CreateMarketRequest = match serde_json::from_value(payload.params) {
                Ok(p) => p,
                Err(e) => return responses::bad_request(&format!("Invalid parameters: {}", e)).into_response(),
            };
            
            // Generate market ID
            let market_id = chrono::Utc::now().timestamp_micros() as u128;
            
            match TransactionBuilder::build_create_market_tx(
                &state.program_id,
                market_id,
                &wallet,
                &params.question,
                &params.outcomes,
                params.end_time,
                params.market_type.unwrap_or(MarketType::Binary),
                params.fee_rate.unwrap_or(250),
            ) {
                Ok(mut tx) => {
                    tx.message.recent_blockhash = recent_blockhash;
                    tx
                },
                Err(e) => return responses::internal_error(&format!("Failed to build transaction: {}", e)).into_response(),
            }
        }
        
        TransactionType::PlaceTrade => {
            let params: PlaceTradeRequest = match serde_json::from_value(payload.params) {
                Ok(p) => p,
                Err(e) => return responses::bad_request(&format!("Invalid parameters: {}", e)).into_response(),
            };
            
            match TransactionBuilder::build_place_trade_tx(
                &state.program_id,
                &wallet,
                params.market_id,
                params.outcome,
                params.amount,
                params.leverage.unwrap_or(1),
            ) {
                Ok(mut tx) => {
                    tx.message.recent_blockhash = recent_blockhash;
                    tx
                },
                Err(e) => return responses::internal_error(&format!("Failed to build transaction: {}", e)).into_response(),
            }
        }
        
        TransactionType::ClosePosition => {
            let params: ClosePositionRequest = match serde_json::from_value(payload.params) {
                Ok(p) => p,
                Err(e) => return responses::bad_request(&format!("Invalid parameters: {}", e)).into_response(),
            };
            
            // Parse market_id from position_id (assuming format is "market_id:owner")
            let market_id: u128 = match params.position_id.split(':').next()
                .and_then(|s| s.parse().ok()) {
                Some(id) => id,
                None => return responses::bad_request("Invalid position ID format").into_response(),
            };
            
            match TransactionBuilder::build_close_position_tx(
                &state.program_id,
                &wallet,
                market_id,
            ) {
                Ok(mut tx) => {
                    tx.message.recent_blockhash = recent_blockhash;
                    tx
                },
                Err(e) => return responses::internal_error(&format!("Failed to build transaction: {}", e)).into_response(),
            }
        }
        
        TransactionType::CreateDemoAccount => {
            // Build create demo account transaction
            let demo_account_pda = get_demo_account_pda(&state.program_id, &wallet);
            
            use solana_sdk::instruction::{AccountMeta, Instruction};
            use crate::types::BettingInstruction;
            use borsh::BorshSerialize;
            
            let instruction = Instruction {
                program_id: state.program_id,
                accounts: vec![
                    AccountMeta::new(demo_account_pda, false),
                    AccountMeta::new(wallet, true),
                    AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                ],
                data: BettingInstruction::CreateDemoAccount.try_to_vec().unwrap(),
            };
            
            let mut tx = Transaction::new_with_payer(
                &[instruction],
                Some(&wallet),
            );
            tx.message.recent_blockhash = recent_blockhash;
            tx
        }
    };
    
    // Prepare transaction for signing
    match prepare_tx(transaction, recent_blockhash).await {
        Ok(prepared) => responses::ok(prepared).into_response(),
        Err(e) => responses::internal_error(&format!("Failed to prepare transaction: {}", e)).into_response(),
    }
}

/// Submit signed transaction
pub async fn submit_transaction(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Json(payload): Json<SubmitTransactionRequest>,
) -> Response {
    // Deserialize transaction
    let transaction = match TransactionBuilder::deserialize_signed_tx(&payload.signed_transaction) {
        Ok(tx) => tx,
        Err(e) => return responses::bad_request(&format!("Invalid transaction: {}", e)).into_response(),
    };
    
    // Verify signatures
    match crate::transaction_signing::verify_transaction_signatures(&transaction) {
        Ok(valid) => {
            if !valid {
                return responses::bad_request("Invalid transaction signatures").into_response();
            }
        }
        Err(e) => return responses::bad_request(&format!("Failed to verify signatures: {}", e)).into_response(),
    }
    
    // Submit transaction
    let result = if payload.wait_for_confirmation {
        state.rpc_client.send_and_confirm_transaction(&transaction)
    } else {
        state.rpc_client.send_transaction(&transaction)
    };
    
    match result {
        Ok(signature) => {
            info!("Transaction submitted: {}", signature);
            
            // Get transaction status if confirmed
            let (confirmed, slot, block_time) = if payload.wait_for_confirmation {
                match state.rpc_client.get_signature_status(&signature) {
                    Ok(Some(status)) => {
                        (
                            status.is_ok(),
                            None, // slot not directly available
                            None, // block_time not available from signature status
                        )
                    }
                    Ok(None) | Err(_) => (false, None, None),
                }
            } else {
                (false, None, None)
            };
            
            responses::ok(TransactionResponse {
                signature: signature.to_string(),
                confirmed,
                slot,
                block_time,
                error: None,
            }).into_response()
        }
        Err(e) => {
            error!("Transaction submission failed: {}", e);
            responses::bad_request(&format!("Transaction submission failed: {}", e)).into_response()
        }
    }
}

/// Get transaction status
pub async fn get_transaction_status(
    State(state): State<AppState>,
    Path(signature_str): Path<String>,
) -> Response {
    let signature = match Signature::from_str(&signature_str) {
        Ok(sig) => sig,
        Err(e) => return responses::bad_request(&format!("Invalid signature: {}", e)).into_response(),
    };
    
    match state.rpc_client.get_signature_status(&signature) {
        Ok(Some(status)) => {
            responses::ok(json!({
                "signature": signature_str,
                "confirmed": status.is_ok(),
                "status": if status.is_ok() { "success" } else { "failed" },
                "error": status.err().map(|e| e.to_string()),
            })).into_response()
        }
        Ok(None) => {
            responses::not_found("Transaction not found").into_response()
        }
        Err(e) => {
            responses::service_unavailable(&format!("Failed to get transaction status: {}", e)).into_response()
        }
    }
}

/// Estimate transaction fee
pub async fn estimate_transaction_fee(
    State(state): State<AppState>,
    Json(payload): Json<PrepareTransactionRequest>,
) -> Response {
    let wallet = match Pubkey::from_str(&payload.wallet) {
        Ok(pk) => pk,
        Err(e) => return responses::bad_request(&format!("Invalid wallet: {}", e)).into_response(),
    };
    
    // Build transaction to estimate fee
    let recent_blockhash = match state.rpc_client.get_latest_blockhash() {
        Ok(bh) => bh,
        Err(e) => return responses::service_unavailable(&format!("Failed to get blockhash: {}", e)).into_response(),
    };
    
    // Build minimal transaction for fee estimation
    let tx = Transaction::new_with_payer(
        &[],
        Some(&wallet),
    );
    
    match state.rpc_client.get_fee_for_message(&tx.message) {
        Ok(fee) => {
            responses::ok(json!({
                "estimated_fee": fee,
                "fee_lamports": fee,
                "fee_sol": fee as f64 / 1_000_000_000.0,
            })).into_response()
        }
        Err(e) => {
            responses::service_unavailable(&format!("Failed to estimate fee: {}", e)).into_response()
        }
    }
}

// Helper function to get demo account PDA
fn get_demo_account_pda(program_id: &Pubkey, owner: &Pubkey) -> Pubkey {
    crate::pda::helpers::demo_account_pda(program_id, owner)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_type_deserialization() {
        let json = r#""create_market""#;
        let tx_type: TransactionType = serde_json::from_str(json).unwrap();
        assert!(matches!(tx_type, TransactionType::CreateMarket));
        
        let json = r#""place_trade""#;
        let tx_type: TransactionType = serde_json::from_str(json).unwrap();
        assert!(matches!(tx_type, TransactionType::PlaceTrade));
    }
    
    #[test]
    fn test_prepare_transaction_request() {
        let json = r#"{
            "transaction_type": "place_trade",
            "params": {
                "market_id": 123,
                "amount": 1000,
                "outcome": 0,
                "leverage": 5
            },
            "wallet": "11111111111111111111111111111111"
        }"#;
        
        let req: PrepareTransactionRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.transaction_type, TransactionType::PlaceTrade));
    }
}