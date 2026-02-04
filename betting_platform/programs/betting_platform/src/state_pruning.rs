use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash;
use crate::account_structs::{ProposalPDA, ProposalState};
use crate::errors::ErrorCode;

pub const PRUNE_GRACE_PERIOD: u64 = 432_000; // ~2 days at 0.4s/slot

pub struct StatePruner;

impl StatePruner {
    // CLAUDE.md: "Auto-prune post-settle_slot (close PDA, reclaim rent)"
    pub fn prune_resolved_markets(
        ctx: Context<PruneMarkets>,
        batch_size: u8,
    ) -> Result<()> {
        let current_slot = Clock::get()?.slot;
        let mut pruned_count = 0;

        for i in 0..ctx.remaining_accounts.len() {
            if pruned_count >= batch_size {
                break;
            }

            if let Ok(proposal) = Account::<ProposalPDA>::try_from(&ctx.remaining_accounts[i]) {
                // Check if ready for pruning (resolved + grace period)
                if proposal.state == ProposalState::Resolved &&
                   current_slot > proposal.settle_slot + PRUNE_GRACE_PERIOD {

                    // Archive to IPFS first
                    emit!(MarketArchived {
                        proposal_id: proposal.proposal_id,
                        ipfs_hash: Self::archive_to_ipfs(&proposal)?,
                        slot: current_slot,
                    });

                    // Reclaim rent to vault
                    let account = &ctx.remaining_accounts[i];
                    let rent_lamports = account.lamports();
                    **account.lamports.borrow_mut() = 0;
                    **ctx.accounts.vault.lamports.borrow_mut() += rent_lamports;

                    pruned_count += 1;
                }
            }
        }

        Ok(())
    }

    fn archive_to_ipfs(proposal: &ProposalPDA) -> Result<[u8; 32]> {
        // In production, serialize and upload to IPFS
        // Return IPFS hash for future retrieval
        let serialized = proposal.try_to_vec()?;
        Ok(hash(&serialized).to_bytes())
    }
}

#[derive(Accounts)]
pub struct PruneMarkets<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// CHECK: Vault account to receive reclaimed rent
    #[account(mut)]
    pub vault: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[event]
pub struct MarketArchived {
    pub proposal_id: [u8; 32],
    pub ipfs_hash: [u8; 32],
    pub slot: u64,
}

// Lookup table for hot verses
pub struct VerseLookupTable {
    pub table_address: Pubkey,
    pub entries: Vec<LookupEntry>,
    pub last_update: u64,
    pub hit_rate: f32,
}

pub struct LookupEntry {
    pub verse_id: u128,
    pub pda_address: Pubkey,
    pub access_count: u32,
    pub last_access: u64,
}

impl VerseLookupTable {
    pub fn optimize_for_hot_verses(&mut self) -> Result<()> {
        // Sort by access frequency
        self.entries.sort_by(|a, b| b.access_count.cmp(&a.access_count));

        // Keep top 256 most accessed verses in lookup table
        self.entries.truncate(256);

        // Update on-chain lookup table
        self.update_onchain_table()?;

        Ok(())
    }
    
    fn update_onchain_table(&self) -> Result<()> {
        // In production, this would update the on-chain address lookup table
        // For now, just return success
        Ok(())
    }
}