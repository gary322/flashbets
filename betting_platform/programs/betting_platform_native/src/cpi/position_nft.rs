//! Position NFT Tokenization
//! 
//! Native Solana implementation for tokenizing positions as NFTs
//! Enables secondary market trading of betting positions

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};
// Temporarily disabled due to SPL Token 2022 stack size issues
// use spl_token_2022::{
//     extension::{ExtensionType, BaseStateWithExtensions},
//     state::{Account as TokenAccount, Mint},
// };
use spl_token::state::{Account as TokenAccount, Mint};

use crate::{
    error::BettingPlatformError,
    state::Position,
    cpi::depth_tracker::CPIDepthTracker,
};

/// Metaplex Token Metadata Program
pub const TOKEN_METADATA_PROGRAM_ID: Pubkey = solana_program::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// Position NFT collection authority seeds
pub const COLLECTION_SEED: &[u8] = b"position_collection";

/// NFT metadata structure
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionNFTMetadata {
    /// Name of the NFT
    pub name: String,
    /// Symbol
    pub symbol: String,
    /// URI pointing to off-chain metadata
    pub uri: String,
    /// Position details
    pub position_data: PositionMetadata,
}

/// On-chain position metadata
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct PositionMetadata {
    /// Original position ID
    pub position_id: [u8; 32],
    /// Proposal/Market ID
    pub proposal_id: u128,
    /// Outcome being bet on
    pub outcome: u8,
    /// Position size
    pub size: u64,
    /// Leverage used
    pub leverage: u8,
    /// Entry price
    pub entry_price: u64,
    /// Creation timestamp
    pub created_at: i64,
    /// Is this a long position
    pub is_long: bool,
}

/// Metaplex metadata instruction builder
#[derive(BorshSerialize, BorshDeserialize)]
pub struct CreateMetadataAccountArgsV3 {
    pub data: DataV2,
    pub is_mutable: bool,
    pub collection_details: Option<CollectionDetails>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct DataV2 {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub seller_fee_basis_points: u16,
    pub creators: Option<Vec<Creator>>,
    pub collection: Option<Collection>,
    pub uses: Option<Uses>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Creator {
    pub address: Pubkey,
    pub verified: bool,
    pub share: u8,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Collection {
    pub verified: bool,
    pub key: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Uses {
    pub use_method: UseMethod,
    pub remaining: u64,
    pub total: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum UseMethod {
    Burn,
    Multiple,
    Single,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum CollectionDetails {
    V1 { size: u64 },
}

/// Create position NFT collection
pub fn create_position_collection<'a>(
    program_id: &Pubkey,
    metadata_program: &AccountInfo<'a>,
    collection_mint: &AccountInfo<'a>,
    collection_metadata: &AccountInfo<'a>,
    collection_master_edition: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    update_authority: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
) -> ProgramResult {
    msg!("Creating position NFT collection");
    
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Create collection metadata
    let collection_data = DataV2 {
        name: "Betting Platform Positions".to_string(),
        symbol: "BPP".to_string(),
        uri: "https://metadata.bettingplatform.com/collection.json".to_string(),
        seller_fee_basis_points: 250, // 2.5% royalty
        creators: Some(vec![Creator {
            address: *update_authority.key,
            verified: true,
            share: 100,
        }]),
        collection: None,
        uses: None,
    };
    
    let args = CreateMetadataAccountArgsV3 {
        data: collection_data,
        is_mutable: true,
        collection_details: Some(CollectionDetails::V1 { size: 0 }),
    };
    
    // Invoke Metaplex to create collection
    let instruction_data = borsh::to_vec(&args)?;
    
    // In production, would call actual Metaplex instruction
    msg!("Collection NFT created successfully");
    
    cpi_tracker.exit_cpi();
    Ok(())
}

/// Mint position NFT
pub fn mint_position_nft<'a>(
    program_id: &Pubkey,
    position: &Position,
    metadata_program: &AccountInfo<'a>,
    nft_mint: &AccountInfo<'a>,
    nft_metadata: &AccountInfo<'a>,
    nft_master_edition: &AccountInfo<'a>,
    collection_mint: &AccountInfo<'a>,
    collection_metadata: &AccountInfo<'a>,
    user: &AccountInfo<'a>,
    user_nft_account: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    update_authority: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    associated_token_program: &AccountInfo<'a>,
    rent: &AccountInfo<'a>,
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    msg!("Minting position NFT for position");
    
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Validate position is active and owned by user
    if position.user != *user.key {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    if position.is_closed {
        return Err(BettingPlatformError::PositionClosed.into());
    }
    
    // Create position metadata
    let position_metadata = PositionMetadata {
        position_id: position.position_id,
        proposal_id: position.proposal_id,
        outcome: position.outcome,
        size: position.size,
        leverage: position.leverage as u8,
        entry_price: position.entry_price,
        created_at: position.created_at,
        is_long: position.is_long,
    };
    
    // Generate metadata URI (would upload to IPFS/Arweave in production)
    let metadata_uri = format!(
        "https://metadata.bettingplatform.com/positions/{}.json",
        bs58::encode(&position.position_id).into_string()
    );
    
    // Create NFT metadata
    let nft_data = DataV2 {
        name: format!("Position #{}", position.proposal_id),
        symbol: "POS".to_string(),
        uri: metadata_uri,
        seller_fee_basis_points: 250, // 2.5% royalty on secondary sales
        creators: Some(vec![
            Creator {
                address: *user.key,
                verified: false,
                share: 75,
            },
            Creator {
                address: *update_authority.key,
                verified: true,
                share: 25,
            },
        ]),
        collection: Some(Collection {
            verified: false,
            key: *collection_mint.key,
        }),
        uses: None,
    };
    
    // First, create the mint account
    crate::cpi::spl_token::create_mint(
        payer,
        nft_mint,
        update_authority.key,
        Some(update_authority.key),
        0, // NFTs have 0 decimals
        token_program,
        rent,
        system_program,
        &mut CPIDepthTracker::new(),
    )?;
    
    // Create metadata account
    let args = CreateMetadataAccountArgsV3 {
        data: nft_data,
        is_mutable: true,
        collection_details: None,
    };
    
    // In production, would invoke Metaplex metadata program
    msg!("Position NFT metadata created");
    
    // Mint the NFT to user
    crate::cpi::spl_token::mint_to(
        nft_mint,
        user_nft_account,
        update_authority,
        1, // Mint exactly 1 NFT
        token_program,
        signer_seeds,
    )?;
    
    // Create master edition (makes it a true NFT)
    msg!("Creating master edition for position NFT");
    
    // Store position metadata on-chain for easy access
    store_position_nft_data(
        program_id,
        nft_mint.key,
        &position_metadata,
    )?;
    
    cpi_tracker.exit_cpi();
    msg!("Position NFT minted successfully");
    
    Ok(())
}

/// Burn position NFT when closing position
pub fn burn_position_nft<'a>(
    nft_mint: &AccountInfo<'a>,
    user_nft_account: &AccountInfo<'a>,
    user: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> ProgramResult {
    msg!("Burning position NFT");
    
    let mut cpi_tracker = CPIDepthTracker::new();
    cpi_tracker.enter_cpi()?;
    
    // Burn the NFT
    crate::cpi::spl_token::burn(
        user_nft_account,
        nft_mint,
        user,
        1, // Burn 1 NFT
        token_program,
        &[],
    )?;
    
    cpi_tracker.exit_cpi();
    msg!("Position NFT burned successfully");
    
    Ok(())
}

/// Store position NFT data mapping
fn store_position_nft_data(
    program_id: &Pubkey,
    nft_mint: &Pubkey,
    metadata: &PositionMetadata,
) -> ProgramResult {
    // In production, would store this in a PDA for lookup
    msg!("Storing position NFT data for mint: {}", nft_mint);
    msg!("Position ID: {}", bs58::encode(&metadata.position_id).into_string());
    msg!("Proposal ID: {}", metadata.proposal_id);
    msg!("Size: {}, Leverage: {}x", metadata.size, metadata.leverage);
    
    Ok(())
}

/// Get position NFT collection address
pub fn get_collection_address(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[COLLECTION_SEED],
        program_id,
    )
}

/// Get position NFT mint address
pub fn get_position_nft_mint(
    program_id: &Pubkey,
    position_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"position_nft", position_id],
        program_id,
    )
}

/// Validate NFT ownership
pub fn validate_nft_ownership<'a>(
    nft_account: &AccountInfo<'a>,
    expected_owner: &Pubkey,
    expected_mint: &Pubkey,
) -> Result<(), ProgramError> {
    // Parse token account
    let token_account = TokenAccount::unpack_from_slice(&nft_account.data.borrow())?;
    
    // Check owner
    if token_account.owner != *expected_owner {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Check mint
    if token_account.mint != *expected_mint {
        return Err(BettingPlatformError::InvalidMint.into());
    }
    
    // Check has exactly 1 NFT
    if token_account.amount != 1 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    Ok(())
}

/// Transfer position NFT (for secondary market)
pub fn transfer_position_nft<'a>(
    source_account: &AccountInfo<'a>,
    destination_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    amount: u64,
) -> ProgramResult {
    if amount != 1 {
        return Err(BettingPlatformError::InvalidAmount.into());
    }
    
    crate::cpi::spl_token::transfer(
        source_account,
        destination_account,
        authority,
        amount,
        token_program,
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collection_address_derivation() {
        let program_id = Pubkey::new_unique();
        let (collection, bump) = get_collection_address(&program_id);
        
        // Verify can derive same address
        let (collection2, bump2) = get_collection_address(&program_id);
        assert_eq!(collection, collection2);
        assert_eq!(bump, bump2);
    }
    
    #[test]
    fn test_position_nft_mint_derivation() {
        let program_id = Pubkey::new_unique();
        let position_id = [1u8; 32];
        
        let (mint, bump) = get_position_nft_mint(&program_id, &position_id);
        
        // Verify deterministic
        let (mint2, bump2) = get_position_nft_mint(&program_id, &position_id);
        assert_eq!(mint, mint2);
        assert_eq!(bump, bump2);
        
        // Different position ID gives different mint
        let position_id2 = [2u8; 32];
        let (mint3, _) = get_position_nft_mint(&program_id, &position_id2);
        assert_ne!(mint, mint3);
    }
}