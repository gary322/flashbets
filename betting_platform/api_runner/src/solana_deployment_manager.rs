//! Smart contract deployment and management system

use std::{
    sync::Arc,
    collections::HashMap,
    path::{Path, PathBuf},
    fs,
    process::Command,
    str::FromStr,
};
use tokio::sync::RwLock;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    system_program,
    transaction::Transaction,
    instruction::Instruction,
    rent::Rent,
    program_pack::Pack,
};
use solana_sdk::{
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use crate::{
    solana_rpc_service::SolanaRpcService,
    solana_transaction_manager::{SolanaTransactionManager, TransactionPriority},
};

/// Deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub program_name: String,
    pub program_path: PathBuf,
    pub buffer_path: Option<PathBuf>,
    pub upgrade_authority: Option<String>,
    pub deploy_authority: Option<String>,
    pub max_data_len: Option<usize>,
    pub skip_fee_check: bool,
    pub use_upgradeable_loader: bool,
}

/// Program deployment info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub program_id: Pubkey,
    pub deployed_at: chrono::DateTime<chrono::Utc>,
    pub deployer: Pubkey,
    pub data_len: usize,
    pub upgrade_authority: Option<Pubkey>,
    pub deployment_slot: u64,
    pub deployment_cost: u64,
}

/// Deployment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStatus {
    NotDeployed,
    Deploying { progress: f32 },
    Deployed { info: DeploymentInfo },
    Failed { error: String },
    Upgrading { progress: f32 },
}

/// Smart contract deployment manager
pub struct SolanaDeploymentManager {
    rpc_service: Arc<SolanaRpcService>,
    tx_manager: Arc<SolanaTransactionManager>,
    deployments: Arc<RwLock<HashMap<String, DeploymentStatus>>>,
    deployment_configs: Arc<RwLock<HashMap<String, DeploymentConfig>>>,
}

impl SolanaDeploymentManager {
    /// Create new deployment manager
    pub fn new(
        rpc_service: Arc<SolanaRpcService>,
        tx_manager: Arc<SolanaTransactionManager>,
    ) -> Self {
        Self {
            rpc_service,
            tx_manager,
            deployments: Arc::new(RwLock::new(HashMap::new())),
            deployment_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Register a program for deployment
    pub async fn register_program(&self, config: DeploymentConfig) -> Result<()> {
        // Validate program path exists
        if !config.program_path.exists() {
            return Err(anyhow::anyhow!("Program file not found: {:?}", config.program_path));
        }
        
        let program_name = config.program_name.clone();
        self.deployment_configs.write().await.insert(program_name.clone(), config);
        self.deployments.write().await.insert(program_name, DeploymentStatus::NotDeployed);
        
        Ok(())
    }
    
    /// Deploy a program
    pub async fn deploy_program(
        &self,
        program_name: &str,
        deployer_keypair: &Keypair,
    ) -> Result<DeploymentInfo> {
        let config = self.deployment_configs.read().await
            .get(program_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Program not registered: {}", program_name))?;
        
        // Update status
        self.deployments.write().await.insert(
            program_name.to_string(),
            DeploymentStatus::Deploying { progress: 0.0 },
        );
        
        info!("Deploying program: {}", program_name);
        
        let result = if config.use_upgradeable_loader {
            self.deploy_upgradeable_program(&config, deployer_keypair).await
        } else {
            self.deploy_regular_program(&config, deployer_keypair).await
        };
        
        match result {
            Ok(info) => {
                self.deployments.write().await.insert(
                    program_name.to_string(),
                    DeploymentStatus::Deployed { info: info.clone() },
                );
                info!("Successfully deployed program {} at {}", program_name, info.program_id);
                Ok(info)
            }
            Err(e) => {
                self.deployments.write().await.insert(
                    program_name.to_string(),
                    DeploymentStatus::Failed { error: e.to_string() },
                );
                error!("Failed to deploy program {}: {}", program_name, e);
                Err(e)
            }
        }
    }
    
    /// Deploy program with upgradeable loader
    async fn deploy_upgradeable_program(
        &self,
        config: &DeploymentConfig,
        deployer_keypair: &Keypair,
    ) -> Result<DeploymentInfo> {
        // Read program data
        let program_data = fs::read(&config.program_path)
            .context("Failed to read program file")?;
        
        let program_len = program_data.len();
        let rent = Rent::default();
        
        // Calculate buffer size
        let buffer_size = UpgradeableLoaderState::size_of_programdata(program_len);
        let buffer_rent = rent.minimum_balance(buffer_size);
        
        // Create buffer account
        let buffer_keypair = Keypair::new();
        let buffer_pubkey = buffer_keypair.pubkey();
        
        info!("Creating buffer account {} ({} bytes)", buffer_pubkey, buffer_size);
        
        // Create buffer account transaction
        let create_buffer_ix = bpf_loader_upgradeable::create_buffer(
            &deployer_keypair.pubkey(),
            &buffer_pubkey,
            &deployer_keypair.pubkey(),
            buffer_rent,
            program_len,
        )?;
        
        let mut create_buffer_tx = self.tx_manager.build_transaction(
            create_buffer_ix,
            &deployer_keypair.pubkey(),
            Some(TransactionPriority::High),
        ).await?;
        
        let blockhash = self.rpc_service.get_latest_blockhash().await?;
        create_buffer_tx.sign(&[deployer_keypair, &buffer_keypair], blockhash);
        
        let create_sig = self.rpc_service.send_and_confirm_transaction(&create_buffer_tx).await?;
        info!("Buffer created: {}", create_sig);
        
        // Write program data to buffer
        self.write_program_data(
            &buffer_pubkey,
            &program_data,
            deployer_keypair,
        ).await?;
        
        // Deploy program
        let program_keypair = Keypair::new();
        let program_id = program_keypair.pubkey();
        
        let upgrade_authority = config.upgrade_authority
            .as_ref()
            .map(|s| Pubkey::from_str(s))
            .transpose()?
            .unwrap_or_else(|| deployer_keypair.pubkey());
        
        let deploy_ix = bpf_loader_upgradeable::deploy_with_max_program_len(
            &deployer_keypair.pubkey(),
            &program_id,
            &buffer_pubkey,
            &upgrade_authority,
            rent.minimum_balance(UpgradeableLoaderState::size_of_program()),
            program_len,
        )?;
        
        let mut deploy_tx = self.tx_manager.build_transaction(
            deploy_ix,
            &deployer_keypair.pubkey(),
            Some(TransactionPriority::High),
        ).await?;
        
        let blockhash = self.rpc_service.get_latest_blockhash().await?;
        deploy_tx.sign(&[deployer_keypair, &program_keypair], blockhash);
        
        let deploy_sig = self.rpc_service.send_and_confirm_transaction(&deploy_tx).await?;
        info!("Program deployed: {} (tx: {})", program_id, deploy_sig);
        
        // Get deployment slot
        let status = self.rpc_service.get_transaction_status(&deploy_sig).await?
            .ok_or_else(|| anyhow::anyhow!("Failed to get deployment transaction status"))?;
        
        Ok(DeploymentInfo {
            program_id,
            deployed_at: chrono::Utc::now(),
            deployer: deployer_keypair.pubkey(),
            data_len: program_len,
            upgrade_authority: Some(upgrade_authority),
            deployment_slot: status.slot.unwrap_or(0),
            deployment_cost: buffer_rent + rent.minimum_balance(UpgradeableLoaderState::size_of_program()),
        })
    }
    
    /// Deploy regular (non-upgradeable) program
    async fn deploy_regular_program(
        &self,
        config: &DeploymentConfig,
        deployer_keypair: &Keypair,
    ) -> Result<DeploymentInfo> {
        // For regular deployment, we need to use solana-keygen and solana program deploy
        // This is a simplified version - in production you'd use the full CLI tools
        
        let program_data = fs::read(&config.program_path)
            .context("Failed to read program file")?;
        
        let program_keypair = Keypair::new();
        let program_id = program_keypair.pubkey();
        
        // This would require calling the Solana CLI tools
        // For now, return a placeholder
        Err(anyhow::anyhow!("Regular program deployment requires Solana CLI tools"))
    }
    
    /// Write program data to buffer in chunks
    async fn write_program_data(
        &self,
        buffer_pubkey: &Pubkey,
        program_data: &[u8],
        authority_keypair: &Keypair,
    ) -> Result<()> {
        const CHUNK_SIZE: usize = 900; // Leave room for instruction overhead
        let chunks: Vec<_> = program_data.chunks(CHUNK_SIZE).collect();
        let total_chunks = chunks.len();
        
        info!("Writing program data in {} chunks", total_chunks);
        
        for (i, chunk) in chunks.iter().enumerate() {
            let offset = i * CHUNK_SIZE;
            let progress = (i as f32 / total_chunks as f32) * 100.0;
            
            debug!("Writing chunk {}/{} ({}%)", i + 1, total_chunks, progress as u32);
            
            let write_ix = bpf_loader_upgradeable::write(
                buffer_pubkey,
                &authority_keypair.pubkey(),
                offset as u32,
                chunk.to_vec(),
            );
            
            let mut write_tx = self.tx_manager.build_transaction(
                vec![write_ix],
                &authority_keypair.pubkey(),
                Some(TransactionPriority::Medium),
            ).await?;
            
            let blockhash = self.rpc_service.get_latest_blockhash().await?;
            write_tx.sign(&[authority_keypair], blockhash);
            
            self.rpc_service.send_and_confirm_transaction(&write_tx).await?;
        }
        
        info!("Program data written successfully");
        Ok(())
    }
    
    /// Upgrade a deployed program
    pub async fn upgrade_program(
        &self,
        program_name: &str,
        new_buffer_pubkey: &Pubkey,
        upgrade_authority_keypair: &Keypair,
    ) -> Result<()> {
        let deployments = self.deployments.read().await;
        let deployment = deployments.get(program_name)
            .ok_or_else(|| anyhow::anyhow!("Program not found: {}", program_name))?;
        
        let info = match deployment {
            DeploymentStatus::Deployed { info } => info.clone(),
            _ => return Err(anyhow::anyhow!("Program not deployed: {}", program_name)),
        };
        
        drop(deployments);
        
        // Update status
        self.deployments.write().await.insert(
            program_name.to_string(),
            DeploymentStatus::Upgrading { progress: 0.0 },
        );
        
        info!("Upgrading program {} ({})", program_name, info.program_id);
        
        // Get program data account
        let (program_data_address, _) = Pubkey::find_program_address(
            &[info.program_id.as_ref()],
            &bpf_loader_upgradeable::id(),
        );
        
        let upgrade_ix = bpf_loader_upgradeable::upgrade(
            &info.program_id,
            new_buffer_pubkey,
            &upgrade_authority_keypair.pubkey(),
            &upgrade_authority_keypair.pubkey(),
        );
        
        let mut upgrade_tx = self.tx_manager.build_transaction(
            vec![upgrade_ix],
            &upgrade_authority_keypair.pubkey(),
            Some(TransactionPriority::High),
        ).await?;
        
        let blockhash = self.rpc_service.get_latest_blockhash().await?;
        upgrade_tx.sign(&[upgrade_authority_keypair], blockhash);
        
        let sig = self.rpc_service.send_and_confirm_transaction(&upgrade_tx).await?;
        
        info!("Program upgraded successfully: {}", sig);
        
        // Update deployment info
        let mut updated_info = info.clone();
        updated_info.deployed_at = chrono::Utc::now();
        
        self.deployments.write().await.insert(
            program_name.to_string(),
            DeploymentStatus::Deployed { info: updated_info },
        );
        
        Ok(())
    }
    
    /// Get deployment status
    pub async fn get_deployment_status(&self, program_name: &str) -> Option<DeploymentStatus> {
        self.deployments.read().await.get(program_name).cloned()
    }
    
    /// Get all deployments
    pub async fn get_all_deployments(&self) -> HashMap<String, DeploymentStatus> {
        self.deployments.read().await.clone()
    }
    
    /// Verify program deployment
    pub async fn verify_deployment(&self, program_id: &Pubkey) -> Result<bool> {
        match self.rpc_service.get_account(program_id).await? {
            Some(account) => {
                // Check if it's a program account
                if account.executable {
                    info!("Program {} is deployed and executable", program_id);
                    Ok(true)
                } else {
                    warn!("Account {} exists but is not executable", program_id);
                    Ok(false)
                }
            }
            None => {
                info!("Program {} not found on chain", program_id);
                Ok(false)
            }
        }
    }
    
    /// Initialize program (call initialize instruction)
    pub async fn initialize_program(
        &self,
        program_id: &Pubkey,
        initializer_keypair: &Keypair,
        init_params: InitializationParams,
    ) -> Result<()> {
        info!("Initializing program {}", program_id);
        
        // Create initialization instruction based on the program's requirements
        let init_ix = self.create_initialization_instruction(
            program_id,
            &initializer_keypair.pubkey(),
            &init_params,
        )?;
        
        let mut init_tx = self.tx_manager.build_transaction(
            vec![init_ix],
            &initializer_keypair.pubkey(),
            Some(TransactionPriority::High),
        ).await?;
        
        let blockhash = self.rpc_service.get_latest_blockhash().await?;
        init_tx.sign(&[initializer_keypair], blockhash);
        
        let sig = self.rpc_service.send_and_confirm_transaction(&init_tx).await?;
        
        info!("Program initialized: {}", sig);
        Ok(())
    }
    
    /// Create initialization instruction
    fn create_initialization_instruction(
        &self,
        program_id: &Pubkey,
        initializer: &Pubkey,
        params: &InitializationParams,
    ) -> Result<Instruction> {
        // This would be customized based on your program's initialization requirements
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(*initializer, true),
            solana_sdk::instruction::AccountMeta::new(params.config_account, false),
            solana_sdk::instruction::AccountMeta::new_readonly(system_program::id(), false),
        ];
        
        // Serialize initialization parameters
        let data = borsh::to_vec(&params)?;
        
        Ok(Instruction {
            program_id: *program_id,
            accounts,
            data,
        })
    }
}

/// Program initialization parameters
#[derive(Debug, Clone, Serialize, Deserialize, borsh::BorshSerialize)]
pub struct InitializationParams {
    pub config_account: Pubkey,
    pub fee_rate: u16,
    pub min_bet_amount: u64,
    pub max_bet_amount: u64,
    pub settlement_delay: i64,
    pub oracle_pubkey: Pubkey,
}

/// Deployment manager status
#[derive(Debug, Serialize)]
pub struct DeploymentManagerStatus {
    pub registered_programs: Vec<String>,
    pub deployments: HashMap<String, DeploymentStatus>,
}

impl SolanaDeploymentManager {
    /// Get deployment manager status
    pub async fn get_status(&self) -> DeploymentManagerStatus {
        DeploymentManagerStatus {
            registered_programs: self.deployment_configs.read().await.keys().cloned().collect(),
            deployments: self.deployments.read().await.clone(),
        }
    }
}

// Re-export for convenience
use borsh::BorshSerialize;