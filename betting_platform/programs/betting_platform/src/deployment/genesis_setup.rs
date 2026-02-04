use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Mint, Token, TokenAccount, MintTo, Transfer},
    associated_token::AssociatedToken,
};
use anchor_lang::solana_program::clock::Clock;
use crate::deployment::errors::{DeploymentError, GenesisError};
use crate::deployment::types::GlobalConfig;
use crate::account_structs::GlobalConfigPDA;

#[derive(Clone, Debug)]
pub struct GenesisConfig {
    pub initial_coverage: f64,
    pub fee_base: u64, // 3bp in basis points
    pub fee_slope: u64, // 25bp in basis points
    pub mmt_supply: u128, // 100M total
    pub emission_per_slot: u64,
    pub season_duration: u64, // 38,880,000 slots (~6 months)
}

#[derive(Clone, Debug)]
pub struct GenesisState {
    pub global_config: Pubkey,
    pub vault: Pubkey,
    pub mmt_mint: Pubkey,
    pub deployment_slot: u64,
    pub initial_coverage: f64,
}

impl GenesisConfig {
    pub fn default() -> Self {
        Self {
            initial_coverage: 0.0,
            fee_base: 3, // 3 basis points
            fee_slope: 25, // 25 basis points
            mmt_supply: 100_000_000 * 10u128.pow(9), // 100M with 9 decimals
            emission_per_slot: 100,
            season_duration: 38_880_000, // ~6 months
        }
    }

    pub fn initialize_protocol(
        &self,
        program_id: Pubkey,
        ctx: &Context<InitializeProtocol>,
    ) -> Result<GenesisState> {
        msg!("Initializing protocol with genesis configuration");
        
        // Validate configuration
        self.validate_config()?;
        
        // Create global config PDA
        let (global_config_pda, global_config_bump) = Pubkey::find_program_address(
            &[b"global_config"],
            &program_id,
        );
        
        // Initialize global config with $0 vault
        let init_params = InitializeParams {
            fee_base: self.fee_base,
            fee_slope: self.fee_slope,
            emission_per_slot: self.emission_per_slot,
            season_duration: self.season_duration,
            initial_coverage: self.initial_coverage,
        };
        
        // Create vault PDA
        let (vault_pda, vault_bump) = Pubkey::find_program_address(
            &[b"vault"],
            &program_id,
        );
        
        // Create MMT token mint
        let mmt_mint = self.create_mmt_token(ctx)?;
        
        // Lock 90M tokens in entropy sink
        self.lock_undecided_tokens(ctx, mmt_mint)?;
        
        // Get current slot
        let clock = Clock::get()?;
        
        Ok(GenesisState {
            global_config: global_config_pda,
            vault: vault_pda,
            mmt_mint,
            deployment_slot: clock.slot,
            initial_coverage: 0.0, // Start at 0 for bootstrap
        })
    }

    pub fn create_mmt_token(
        &self,
        ctx: &Context<InitializeProtocol>,
    ) -> Result<Pubkey> {
        msg!("Creating MMT token mint");
        
        let mmt_mint = &ctx.accounts.mmt_mint;
        
        // Initialize mint with 9 decimals
        token::initialize_mint(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::InitializeMint {
                    mint: mmt_mint.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
            ),
            9, // decimals
            &ctx.accounts.payer.key(),
            Some(&ctx.accounts.payer.key()),
        )?;
        
        // Mint total supply to treasury
        let treasury_ata = &ctx.accounts.treasury_ata;
        
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: mmt_mint.to_account_info(),
                    to: treasury_ata.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            self.mmt_supply as u64,
        )?;
        
        msg!("Minted {} MMT tokens to treasury", self.mmt_supply);
        
        Ok(mmt_mint.key())
    }

    pub fn lock_undecided_tokens(
        &self,
        ctx: &Context<InitializeProtocol>,
        mmt_mint: Pubkey,
    ) -> Result<()> {
        msg!("Locking 90M tokens in entropy sink");
        
        let entropy_sink = &ctx.accounts.entropy_sink;
        let treasury_ata = &ctx.accounts.treasury_ata;
        
        // Calculate amount to lock (90M tokens)
        let lock_amount = 90_000_000 * 10u64.pow(9);
        
        // Transfer tokens to entropy sink
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: treasury_ata.to_account_info(),
                    to: entropy_sink.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            lock_amount,
        )?;
        
        msg!("Locked {} MMT tokens in entropy sink", lock_amount);
        
        Ok(())
    }

    fn validate_config(&self) -> Result<()> {
        if self.fee_base == 0 || self.fee_slope == 0 {
            return Err(GenesisError::InvalidConfiguration.into());
        }
        
        if self.mmt_supply != 100_000_000 * 10u128.pow(9) {
            return Err(GenesisError::InvalidConfiguration.into());
        }
        
        if self.season_duration < 1_000_000 {
            return Err(GenesisError::InvalidConfiguration.into());
        }
        
        Ok(())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.initial_coverage.to_le_bytes());
        bytes.extend_from_slice(&self.fee_base.to_le_bytes());
        bytes.extend_from_slice(&self.fee_slope.to_le_bytes());
        bytes.extend_from_slice(&self.mmt_supply.to_le_bytes());
        bytes.extend_from_slice(&self.emission_per_slot.to_le_bytes());
        bytes.extend_from_slice(&self.season_duration.to_le_bytes());
        bytes
    }
}

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + GlobalConfigPDA::LEN,
        seeds = [b"global_config"],
        bump
    )]
    pub global_config: Account<'info, GlobalConfigPDA>,
    
    #[account(
        init,
        payer = payer,
        mint::decimals = 9,
        mint::authority = payer,
        seeds = [b"mmt_mint"],
        bump
    )]
    pub mmt_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = payer,
        associated_token::mint = mmt_mint,
        associated_token::authority = payer,
    )]
    pub treasury_ata: Account<'info, TokenAccount>,
    
    #[account(
        init,
        payer = payer,
        token::mint = mmt_mint,
        token::authority = entropy_sink,
        seeds = [b"entropy_sink"],
        bump
    )]
    pub entropy_sink: Account<'info, TokenAccount>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Clone)]
pub struct InitializeParams {
    pub fee_base: u64,
    pub fee_slope: u64,
    pub emission_per_slot: u64,
    pub season_duration: u64,
    pub initial_coverage: f64,
}


pub fn create_initialize_instruction(
    program_id: Pubkey,
    global_config_pda: Pubkey,
    payer: Pubkey,
    params_bytes: Vec<u8>,
) -> anchor_lang::solana_program::instruction::Instruction {
    anchor_lang::solana_program::instruction::Instruction {
        program_id,
        accounts: vec![
            anchor_lang::solana_program::instruction::AccountMeta::new(payer, true),
            anchor_lang::solana_program::instruction::AccountMeta::new(global_config_pda, false),
        ],
        data: params_bytes,
    }
}