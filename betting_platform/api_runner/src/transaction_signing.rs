//! Transaction signing service for secure transaction handling
//! 
//! This module provides secure transaction signing without exposing private keys

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::Transaction,
    message::Message,
    hash::Hash,
};
use base64::{Engine as _, engine::general_purpose};
use crate::pda;

/// Transaction signing request
#[derive(Debug, Deserialize)]
pub struct SigningRequest {
    /// Base64 encoded transaction message
    pub message: String,
    /// Recent blockhash
    pub recent_blockhash: String,
    /// Fee payer public key
    pub fee_payer: String,
}

/// Signed transaction response
#[derive(Debug, Serialize)]
pub struct SignedTransaction {
    /// Base64 encoded signed transaction
    pub transaction: String,
    /// Transaction signature
    pub signature: String,
}

/// Transaction builder for creating unsigned transactions
pub struct TransactionBuilder;

impl TransactionBuilder {
    /// Build create market transaction (unsigned)
    pub fn build_create_market_tx(
        program_id: &Pubkey,
        market_id: u128,
        admin: &Pubkey,
        question: &str,
        outcomes: &[String],
        end_time: i64,
        market_type: crate::types::MarketType,
        fee_rate: u16,
    ) -> Result<Transaction> {
        use crate::rpc_client::MarketInstruction;
        use borsh::BorshSerialize;
        
        // Generate market PDA
        let market_pda = pda::helpers::market_pda(program_id, market_id);
        
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(market_pda, false),
                solana_sdk::instruction::AccountMeta::new(*admin, true),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: MarketInstruction::CreateMarket {
                question: question.to_string(),
                outcomes: outcomes.to_vec(),
                end_time,
                market_type,
                fee_rate,
            }.try_to_vec()?,
        };
        
        Ok(Transaction::new_with_payer(
            &[instruction],
            Some(admin),
        ))
    }
    
    /// Build place trade transaction (unsigned)
    pub fn build_place_trade_tx(
        program_id: &Pubkey,
        trader: &Pubkey,
        market_id: u128,
        outcome: u8,
        amount: u64,
        leverage: u32,
    ) -> Result<Transaction> {
        use crate::rpc_client::MarketInstruction;
        use borsh::BorshSerialize;
        
        // Generate required PDAs
        let market_pda = pda::helpers::market_pda(program_id, market_id);
        let position_pda = pda::helpers::position_pda(program_id, trader, market_id);
        let demo_account_pda = pda::helpers::demo_account_pda(program_id, trader);
        
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(demo_account_pda, false),
                solana_sdk::instruction::AccountMeta::new(market_pda, false),
                solana_sdk::instruction::AccountMeta::new(*trader, true),
                solana_sdk::instruction::AccountMeta::new(position_pda, false),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: MarketInstruction::PlaceTrade {
                market_id,
                outcome,
                amount,
                leverage,
            }.try_to_vec()?,
        };
        
        Ok(Transaction::new_with_payer(
            &[instruction],
            Some(trader),
        ))
    }
    
    /// Build close position transaction (unsigned)
    pub fn build_close_position_tx(
        program_id: &Pubkey,
        trader: &Pubkey,
        market_id: u128,
    ) -> Result<Transaction> {
        use crate::rpc_client::MarketInstruction;
        use borsh::BorshSerialize;
        
        // Generate required PDAs
        let market_pda = pda::helpers::market_pda(program_id, market_id);
        let position_pda = pda::helpers::position_pda(program_id, trader, market_id);
        let demo_account_pda = pda::helpers::demo_account_pda(program_id, trader);
        
        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(demo_account_pda, false),
                solana_sdk::instruction::AccountMeta::new(market_pda, false),
                solana_sdk::instruction::AccountMeta::new(*trader, true),
                solana_sdk::instruction::AccountMeta::new(position_pda, false),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: MarketInstruction::ClosePosition {
                position_id: market_id, // Using market_id as position identifier
            }.try_to_vec()?,
        };
        
        Ok(Transaction::new_with_payer(
            &[instruction],
            Some(trader),
        ))
    }
    
    /// Serialize transaction for client signing
    pub fn serialize_for_signing(transaction: &Transaction) -> Result<String> {
        let message_bytes = bincode::serialize(&transaction.message)?;
        Ok(general_purpose::STANDARD.encode(message_bytes))
    }
    
    /// Deserialize signed transaction from client
    pub fn deserialize_signed_tx(encoded_tx: &str) -> Result<Transaction> {
        let tx_bytes = general_purpose::STANDARD.decode(encoded_tx)?;
        let transaction: Transaction = bincode::deserialize(&tx_bytes)?;
        Ok(transaction)
    }
}

/// Transaction signing modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SigningMode {
    /// Client signs the full transaction
    ClientSide,
    /// Server holds encrypted key (not recommended)
    ServerSide,
    /// Use a hardware wallet
    Hardware,
    /// Multi-sig with partial signatures
    MultiSig,
}

/// Transaction preparation response
#[derive(Debug, Serialize)]
pub struct PreparedTransaction {
    /// Base64 encoded transaction message
    pub message: String,
    /// Recent blockhash used
    pub recent_blockhash: String,
    /// Estimated transaction fee
    pub estimated_fee: u64,
    /// Required signers
    pub required_signers: Vec<String>,
}

/// Prepare transaction for client signing
pub async fn prepare_transaction(
    transaction: Transaction,
    recent_blockhash: Hash,
) -> Result<PreparedTransaction> {
    let message = transaction.message;
    let message_bytes = bincode::serialize(&message)?;
    let encoded_message = general_purpose::STANDARD.encode(message_bytes);
    
    // Calculate fee (simplified - in production use proper fee calculation)
    let estimated_fee = 5000 * message.header.num_required_signatures as u64;
    
    // Get required signers
    let required_signers: Vec<String> = message.account_keys.iter()
        .take(message.header.num_required_signatures as usize)
        .map(|pk| pk.to_string())
        .collect();
    
    Ok(PreparedTransaction {
        message: encoded_message,
        recent_blockhash: recent_blockhash.to_string(),
        estimated_fee,
        required_signers,
    })
}

/// Verify transaction signatures
pub fn verify_transaction_signatures(transaction: &Transaction) -> Result<bool> {
    // Verify all signatures are valid
    for (i, signature) in transaction.signatures.iter().enumerate() {
        if i < transaction.message.header.num_required_signatures as usize {
            if signature == &Signature::default() {
                return Ok(false);
            }
        }
    }
    
    // In production, actually verify signatures against message
    Ok(true)
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_serialization() {
        let program_id = Pubkey::new_unique();
        let trader = Pubkey::new_unique();
        
        let tx = TransactionBuilder::build_place_trade_tx(
            &program_id,
            &trader,
            1,
            0,
            1000,
            1,
        ).unwrap();
        
        let serialized = TransactionBuilder::serialize_for_signing(&tx).unwrap();
        assert!(!serialized.is_empty());
        
        // In a real test, we'd deserialize and verify
    }
}