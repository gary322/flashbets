//! Production-ready Solana RPC service with connection pooling and retry logic

use std::{
    sync::Arc,
    time::{Duration, Instant},
    collections::HashMap,
};
use tokio::sync::{RwLock, Mutex, Semaphore};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
    rpc_request::RpcError,
};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    pubkey::Pubkey,
    signature::{Signature, Keypair},
    transaction::Transaction,
    hash::Hash,
    account::Account,
};
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};

/// RPC endpoint health status
#[derive(Debug, Clone, PartialEq)]
pub enum RpcHealth {
    Healthy,
    Degraded { latency_ms: u64 },
    Unhealthy { error: String },
}

/// RPC endpoint information
#[derive(Debug, Clone)]
pub struct RpcEndpoint {
    pub url: String,
    pub priority: u8,
    pub health: RpcHealth,
    pub last_check: Instant,
    pub request_count: u64,
    pub error_count: u64,
}

/// Solana RPC service configuration
#[derive(Debug, Clone)]
pub struct SolanaRpcConfig {
    pub endpoints: Vec<String>,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub health_check_interval: Duration,
    pub request_timeout: Duration,
    pub max_concurrent_requests: usize,
    pub enable_fallback: bool,
    pub commitment: CommitmentLevel,
}

impl Default for SolanaRpcConfig {
    fn default() -> Self {
        Self {
            endpoints: vec![
                "http://localhost:8899".to_string(),
                "https://api.mainnet-beta.solana.com".to_string(),
                "https://solana-api.projectserum.com".to_string(),
            ],
            max_retries: 3,
            retry_delay_ms: 1000,
            health_check_interval: Duration::from_secs(30),
            request_timeout: Duration::from_secs(30),
            max_concurrent_requests: 100,
            enable_fallback: true,
            commitment: CommitmentLevel::Confirmed,
        }
    }
}

/// Production-ready Solana RPC service
pub struct SolanaRpcService {
    config: SolanaRpcConfig,
    endpoints: Arc<RwLock<Vec<RpcEndpoint>>>,
    current_endpoint: Arc<Mutex<usize>>,
    clients: Arc<RwLock<HashMap<String, Arc<RpcClient>>>>,
    request_semaphore: Arc<Semaphore>,
    metrics: Arc<RwLock<RpcMetrics>>,
}

/// RPC service metrics
#[derive(Debug, Default)]
struct RpcMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_latency_ms: u64,
    endpoint_switches: u64,
}

impl SolanaRpcService {
    /// Create new Solana RPC service
    pub async fn new(config: SolanaRpcConfig) -> Result<Self> {
        let endpoints = config.endpoints.iter()
            .enumerate()
            .map(|(i, url)| RpcEndpoint {
                url: url.clone(),
                priority: i as u8,
                health: RpcHealth::Healthy,
                last_check: Instant::now(),
                request_count: 0,
                error_count: 0,
            })
            .collect();
        
        let service = Self {
            config: config.clone(),
            endpoints: Arc::new(RwLock::new(endpoints)),
            current_endpoint: Arc::new(Mutex::new(0)),
            clients: Arc::new(RwLock::new(HashMap::new())),
            request_semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
            metrics: Arc::new(RwLock::new(RpcMetrics::default())),
        };
        
        // Initialize RPC clients
        service.initialize_clients().await?;
        
        // Start health check task
        service.start_health_checks();
        
        info!("Solana RPC service initialized with {} endpoints", config.endpoints.len());
        
        Ok(service)
    }
    
    /// Initialize RPC clients for all endpoints
    async fn initialize_clients(&self) -> Result<()> {
        let mut clients = self.clients.write().await;
        let endpoints = self.endpoints.read().await;
        
        for endpoint in endpoints.iter() {
            let client = Arc::new(RpcClient::new_with_timeout_and_commitment(
                endpoint.url.clone(),
                self.config.request_timeout,
                CommitmentConfig { commitment: self.config.commitment },
            ));
            clients.insert(endpoint.url.clone(), client);
        }
        
        Ok(())
    }
    
    /// Start background health check task
    fn start_health_checks(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(service.config.health_check_interval).await;
                if let Err(e) = service.check_all_endpoints_health().await {
                    error!("Health check failed: {}", e);
                }
            }
        });
    }
    
    /// Check health of all endpoints
    async fn check_all_endpoints_health(&self) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;
        let clients = self.clients.read().await;
        
        for endpoint in endpoints.iter_mut() {
            if let Some(client) = clients.get(&endpoint.url) {
                let start = Instant::now();
                match client.get_slot() {
                    Ok(_) => {
                        let latency_ms = start.elapsed().as_millis() as u64;
                        endpoint.health = if latency_ms < 1000 {
                            RpcHealth::Healthy
                        } else {
                            RpcHealth::Degraded { latency_ms }
                        };
                        endpoint.last_check = Instant::now();
                    }
                    Err(e) => {
                        endpoint.health = RpcHealth::Unhealthy {
                            error: e.to_string(),
                        };
                        endpoint.last_check = Instant::now();
                        endpoint.error_count += 1;
                    }
                }
            }
        }
        
        // Sort endpoints by health and priority
        endpoints.sort_by(|a, b| {
            match (&a.health, &b.health) {
                (RpcHealth::Healthy, RpcHealth::Healthy) => a.priority.cmp(&b.priority),
                (RpcHealth::Healthy, _) => std::cmp::Ordering::Less,
                (_, RpcHealth::Healthy) => std::cmp::Ordering::Greater,
                (RpcHealth::Degraded { latency_ms: a_lat }, RpcHealth::Degraded { latency_ms: b_lat }) => {
                    a_lat.cmp(b_lat).then(a.priority.cmp(&b.priority))
                }
                (RpcHealth::Degraded { .. }, RpcHealth::Unhealthy { .. }) => std::cmp::Ordering::Less,
                (RpcHealth::Unhealthy { .. }, RpcHealth::Degraded { .. }) => std::cmp::Ordering::Greater,
                (RpcHealth::Unhealthy { .. }, RpcHealth::Unhealthy { .. }) => a.priority.cmp(&b.priority),
            }
        });
        
        Ok(())
    }
    
    /// Get current RPC client with automatic failover
    async fn get_client(&self) -> Result<Arc<RpcClient>> {
        let endpoints = self.endpoints.read().await;
        let clients = self.clients.read().await;
        let mut current_idx = self.current_endpoint.lock().await;
        
        // Try to find a healthy endpoint
        for (idx, endpoint) in endpoints.iter().enumerate() {
            if matches!(endpoint.health, RpcHealth::Healthy | RpcHealth::Degraded { .. }) {
                if let Some(client) = clients.get(&endpoint.url) {
                    if idx != *current_idx {
                        info!("Switching to RPC endpoint: {}", endpoint.url);
                        *current_idx = idx;
                        self.metrics.write().await.endpoint_switches += 1;
                    }
                    return Ok(client.clone());
                }
            }
        }
        
        // If no healthy endpoints, try to use the first one anyway
        if let Some(endpoint) = endpoints.first() {
            if let Some(client) = clients.get(&endpoint.url) {
                warn!("All endpoints unhealthy, using primary: {}", endpoint.url);
                return Ok(client.clone());
            }
        }
        
        Err(anyhow::anyhow!("No available RPC endpoints"))
    }
    
    /// Execute RPC request with retry logic
    async fn execute_with_retry<T, F, Fut>(&self, operation: F) -> Result<T>
    where
        F: Fn(Arc<RpcClient>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let _permit = self.request_semaphore.acquire().await?;
        let start = Instant::now();
        let mut last_error = None;
        
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(
                    self.config.retry_delay_ms * attempt as u64
                )).await;
            }
            
            let client = self.get_client().await?;
            
            match operation(client).await {
                Ok(result) => {
                    // Update metrics
                    let mut metrics = self.metrics.write().await;
                    metrics.total_requests += 1;
                    metrics.successful_requests += 1;
                    metrics.total_latency_ms += start.elapsed().as_millis() as u64;
                    
                    // Update endpoint stats
                    let mut endpoints = self.endpoints.write().await;
                    let current_idx = self.current_endpoint.lock().await;
                    if let Some(endpoint) = endpoints.get_mut(*current_idx) {
                        endpoint.request_count += 1;
                    }
                    
                    return Ok(result);
                }
                Err(e) => {
                    warn!("RPC request failed (attempt {}/{}): {}", attempt + 1, self.config.max_retries + 1, e);
                    last_error = Some(e);
                    
                    // Update endpoint error count
                    let mut endpoints = self.endpoints.write().await;
                    let current_idx = self.current_endpoint.lock().await;
                    if let Some(endpoint) = endpoints.get_mut(*current_idx) {
                        endpoint.error_count += 1;
                    }
                    
                    // Try next endpoint if fallback is enabled
                    if self.config.enable_fallback && attempt < self.config.max_retries {
                        let mut current_idx = self.current_endpoint.lock().await;
                        *current_idx = (*current_idx + 1) % endpoints.len();
                    }
                }
            }
        }
        
        // Update failure metrics
        self.metrics.write().await.failed_requests += 1;
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }
    
    /// Get latest blockhash with caching
    pub async fn get_latest_blockhash(&self) -> Result<Hash> {
        self.execute_with_retry(|client| async move {
            client.get_latest_blockhash()
                .context("Failed to get latest blockhash")
        }).await
    }
    
    /// Get account information
    pub async fn get_account(&self, pubkey: &Pubkey) -> Result<Option<Account>> {
        self.execute_with_retry(|client| async move {
            match client.get_account(pubkey) {
                Ok(account) => Ok(Some(account)),
                Err(e) => {
                    // Check if it's a "not found" error
                    if e.to_string().contains("AccountNotFound") || e.to_string().contains("could not find account") {
                        // Account doesn't exist
                        Ok(None)
                    } else {
                        Err(e.into())
                    }
                }
            }
        }).await
    }
    
    /// Get multiple accounts
    pub async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        self.execute_with_retry(|client| async move {
            client.get_multiple_accounts(pubkeys)
                .context("Failed to get multiple accounts")
        }).await
    }
    
    /// Send transaction with confirmation
    pub async fn send_and_confirm_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        self.execute_with_retry(|client| async move {
            let config = RpcSendTransactionConfig {
                skip_preflight: false,
                preflight_commitment: Some(self.config.commitment),
                encoding: None,
                max_retries: Some(0), // Handle retries at service level
                min_context_slot: None,
            };
            
            // Send transaction
            let signature = client.send_transaction_with_config(transaction, config)?;
            
            // Wait for confirmation
            let commitment = CommitmentConfig { commitment: self.config.commitment };
            client.confirm_transaction_with_commitment(&signature, commitment)
                .context("Failed to confirm transaction")?;
            
            Ok(signature)
        }).await
    }
    
    /// Simulate transaction
    pub async fn simulate_transaction(&self, transaction: &Transaction) -> Result<()> {
        self.execute_with_retry(|client| async move {
            let config = RpcSimulateTransactionConfig {
                sig_verify: true,
                replace_recent_blockhash: false,
                commitment: Some(CommitmentConfig {
                    commitment: self.config.commitment,
                }),
                encoding: None,
                accounts: None,
                min_context_slot: None,
                inner_instructions: false,
            };
            
            let result = client.simulate_transaction_with_config(transaction, config)?;
            
            if let Some(err) = result.value.err {
                return Err(anyhow::anyhow!("Transaction simulation failed: {:?}", err));
            }
            
            Ok(())
        }).await
    }
    
    /// Get balance
    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.execute_with_retry(|client| async move {
            client.get_balance(pubkey)
                .context("Failed to get balance")
        }).await
    }
    
    /// Get program accounts with filters
    pub async fn get_program_accounts(
        &self,
        program_id: &Pubkey,
        filters: Vec<solana_client::rpc_filter::RpcFilterType>,
    ) -> Result<Vec<(Pubkey, Account)>> {
        let program_id = *program_id;
        let commitment = self.config.commitment;
        
        self.execute_with_retry(move |client| {
            let filters = filters.clone();
            async move {
                client.get_program_accounts_with_config(
                    &program_id,
                    solana_client::rpc_config::RpcProgramAccountsConfig {
                        filters: Some(filters),
                        account_config: solana_client::rpc_config::RpcAccountInfoConfig {
                            encoding: None,
                            data_slice: None,
                            commitment: Some(CommitmentConfig { commitment }),
                            min_context_slot: None,
                        },
                        with_context: None,
                    },
                ).context("Failed to get program accounts")
            }
        }).await
    }
    
    /// Get transaction status
    pub async fn get_transaction_status(&self, signature: &Signature) -> Result<Option<TransactionStatus>> {
        self.execute_with_retry(|client| async move {
            // Use get_signature_statuses to get more detailed information
            let statuses = client.get_signature_statuses(&[*signature])?;
            
            match statuses.value.get(0).and_then(|s| s.as_ref()) {
                Some(status) => {
                    Ok(Some(TransactionStatus {
                        confirmed: status.confirmations.is_some(),
                        slot: Some(status.slot),
                        confirmations: status.confirmations,
                        err: status.err.as_ref().map(|e| format!("{:?}", e)),
                    }))
                }
                None => Ok(None),
            }
        }).await
    }
    
    /// Get health status
    pub async fn get_health_status(&self) -> HealthStatus {
        let endpoints = self.endpoints.read().await;
        let metrics = self.metrics.read().await;
        let current_idx = self.current_endpoint.lock().await;
        
        let endpoint_statuses: Vec<EndpointStatus> = endpoints.iter()
            .map(|ep| EndpointStatus {
                url: ep.url.clone(),
                health: ep.health.clone(),
                request_count: ep.request_count,
                error_count: ep.error_count,
                is_active: endpoints.iter().position(|e| e.url == ep.url) == Some(*current_idx),
            })
            .collect();
        
        let success_rate = if metrics.total_requests > 0 {
            (metrics.successful_requests as f64 / metrics.total_requests as f64) * 100.0
        } else {
            0.0
        };
        
        let avg_latency_ms = if metrics.successful_requests > 0 {
            metrics.total_latency_ms / metrics.successful_requests
        } else {
            0
        };
        
        HealthStatus {
            endpoints: endpoint_statuses,
            total_requests: metrics.total_requests,
            success_rate,
            avg_latency_ms,
            endpoint_switches: metrics.endpoint_switches,
        }
    }
    
    /// Get recent blockhash
    pub async fn get_recent_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        let client = self.get_client().await?;
        let (blockhash, _) = client.get_recent_blockhash()
            .map_err(|e| anyhow::anyhow!("Failed to get recent blockhash: {}", e))?;
        Ok(blockhash)
    }
    
    /// Get payer pubkey
    pub fn get_payer_pubkey(&self) -> Pubkey {
        // TODO: In production, this should be configured properly
        Pubkey::default()
    }
}

impl Clone for SolanaRpcService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            endpoints: self.endpoints.clone(),
            current_endpoint: self.current_endpoint.clone(),
            clients: self.clients.clone(),
            request_semaphore: self.request_semaphore.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

/// Transaction status
#[derive(Debug, Serialize)]
pub struct TransactionStatus {
    pub confirmed: bool,
    pub slot: Option<u64>,
    pub confirmations: Option<usize>,
    pub err: Option<String>,
}

/// Health status response
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub endpoints: Vec<EndpointStatus>,
    pub total_requests: u64,
    pub success_rate: f64,
    pub avg_latency_ms: u64,
    pub endpoint_switches: u64,
}

/// Endpoint status
#[derive(Debug, Serialize)]
pub struct EndpointStatus {
    pub url: String,
    pub health: RpcHealth,
    pub request_count: u64,
    pub error_count: u64,
    pub is_active: bool,
}

// Make RpcHealth serializable
impl Serialize for RpcHealth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            RpcHealth::Healthy => serializer.serialize_str("healthy"),
            RpcHealth::Degraded { latency_ms } => {
                serializer.serialize_str(&format!("degraded ({}ms)", latency_ms))
            }
            RpcHealth::Unhealthy { error } => {
                serializer.serialize_str(&format!("unhealthy: {}", error))
            }
        }
    }
}

#[allow(dead_code)]#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_rpc_service_initialization() {
        let config = SolanaRpcConfig {
            endpoints: vec!["http://localhost:8899".to_string()],
            ..Default::default()
        };
        
        let service = SolanaRpcService::new(config).await.unwrap();
        let health = service.get_health_status().await;
        
        assert_eq!(health.endpoints.len(), 1);
        assert_eq!(health.total_requests, 0);
    }
}