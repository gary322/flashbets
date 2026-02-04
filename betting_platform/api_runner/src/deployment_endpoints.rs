//! Smart contract deployment endpoints

use axum::{
    extract::{State, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Keypair,
};
use std::{str::FromStr, path::PathBuf};
use tracing::{info, warn, error};

use crate::{
    AppState,
    jwt_validation::AuthenticatedUser,
    rbac_authorization::{Permission, AuthorizationService},
    solana_deployment_manager::{
        DeploymentConfig, DeploymentInfo, DeploymentStatus,
        InitializationParams, DeploymentManagerStatus,
    },
};

/// Program registration request
#[derive(Debug, Deserialize)]
pub struct RegisterProgramRequest {
    pub program_name: String,
    pub program_path: String,
    pub buffer_path: Option<String>,
    pub upgrade_authority: Option<String>,
    pub deploy_authority: Option<String>,
    pub max_data_len: Option<usize>,
    pub skip_fee_check: Option<bool>,
    pub use_upgradeable_loader: Option<bool>,
}

/// Program deployment request
#[derive(Debug, Deserialize)]
pub struct DeployProgramRequest {
    pub program_name: String,
    pub deployer_keypair: String, // Base58 encoded private key
}

/// Program upgrade request
#[derive(Debug, Deserialize)]
pub struct UpgradeProgramRequest {
    pub program_name: String,
    pub new_buffer_pubkey: String,
    pub upgrade_authority_keypair: String, // Base58 encoded private key
}

/// Program initialization request
#[derive(Debug, Deserialize)]
pub struct InitializeProgramRequest {
    pub program_id: String,
    pub initializer_keypair: String, // Base58 encoded private key
    pub config_account: String,
    pub fee_rate: u16,
    pub min_bet_amount: u64,
    pub max_bet_amount: u64,
    pub settlement_delay: i64,
    pub oracle_pubkey: String,
}

/// Deployment verification response
#[derive(Debug, Serialize)]
pub struct VerificationResponse {
    pub program_id: String,
    pub is_deployed: bool,
    pub is_executable: bool,
    pub account_exists: bool,
}

/// Register a program for deployment (Admin only)
pub async fn register_program(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<RegisterProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check admin permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::UpdateSystemConfig) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let config = DeploymentConfig {
        program_name: request.program_name.clone(),
        program_path: PathBuf::from(request.program_path),
        buffer_path: request.buffer_path.map(PathBuf::from),
        upgrade_authority: request.upgrade_authority,
        deploy_authority: request.deploy_authority,
        max_data_len: request.max_data_len,
        skip_fee_check: request.skip_fee_check.unwrap_or(false),
        use_upgradeable_loader: request.use_upgradeable_loader.unwrap_or(true),
    };
    
    match deployment_manager.register_program(config).await {
        Ok(()) => {
            info!("Program {} registered for deployment", request.program_name);
            Ok(Json(serde_json::json!({
                "success": true,
                "program_name": request.program_name,
                "message": "Program registered successfully"
            })))
        }
        Err(e) => {
            error!("Failed to register program: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Deploy a registered program (Admin only)
pub async fn deploy_program(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<DeployProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check admin permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::UpdateSystemConfig) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Parse deployer keypair
    let deployer_bytes = bs58::decode(&request.deployer_keypair)
        .into_vec()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let deployer_keypair = Keypair::from_bytes(&deployer_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match deployment_manager.deploy_program(&request.program_name, &deployer_keypair).await {
        Ok(info) => {
            info!("Program {} deployed at {}", request.program_name, info.program_id);
            
            // Log deployment event
            state.security_logger.log_auth_event(
                &auth.claims.wallet,
                "program_deployed",
                Some(&format!("Program: {}, ID: {}", request.program_name, info.program_id)),
            ).await;
            
            Ok(Json(info))
        }
        Err(e) => {
            error!("Failed to deploy program: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Upgrade a deployed program (Admin only)
pub async fn upgrade_program(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<UpgradeProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check admin permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::UpdateSystemConfig) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Parse buffer pubkey
    let buffer_pubkey = Pubkey::from_str(&request.new_buffer_pubkey)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Parse upgrade authority keypair
    let authority_bytes = bs58::decode(&request.upgrade_authority_keypair)
        .into_vec()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let authority_keypair = Keypair::from_bytes(&authority_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match deployment_manager.upgrade_program(
        &request.program_name,
        &buffer_pubkey,
        &authority_keypair,
    ).await {
        Ok(()) => {
            info!("Program {} upgraded successfully", request.program_name);
            
            // Log upgrade event
            state.security_logger.log_auth_event(
                &auth.claims.wallet,
                "program_upgraded",
                Some(&format!("Program: {}", request.program_name)),
            ).await;
            
            Ok(Json(serde_json::json!({
                "success": true,
                "program_name": request.program_name,
                "message": "Program upgraded successfully"
            })))
        }
        Err(e) => {
            error!("Failed to upgrade program: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Initialize a deployed program (Admin only)
pub async fn initialize_program(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<InitializeProgramRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check admin permission
    let user_role = crate::auth::UserRole::from_str(&auth.claims.role).unwrap_or(crate::auth::UserRole::User);
    if !state.authorization_service.has_permission(&user_role, &Permission::UpdateSystemConfig) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    // Parse program ID
    let program_id = Pubkey::from_str(&request.program_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Parse initializer keypair
    let initializer_bytes = bs58::decode(&request.initializer_keypair)
        .into_vec()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let initializer_keypair = Keypair::from_bytes(&initializer_bytes)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Parse other pubkeys
    let config_account = Pubkey::from_str(&request.config_account)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let oracle_pubkey = Pubkey::from_str(&request.oracle_pubkey)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let init_params = InitializationParams {
        config_account,
        fee_rate: request.fee_rate,
        min_bet_amount: request.min_bet_amount,
        max_bet_amount: request.max_bet_amount,
        settlement_delay: request.settlement_delay,
        oracle_pubkey,
    };
    
    match deployment_manager.initialize_program(
        &program_id,
        &initializer_keypair,
        init_params,
    ).await {
        Ok(()) => {
            info!("Program {} initialized successfully", program_id);
            
            Ok(Json(serde_json::json!({
                "success": true,
                "program_id": program_id.to_string(),
                "message": "Program initialized successfully"
            })))
        }
        Err(e) => {
            error!("Failed to initialize program: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get deployment status for a program
pub async fn get_deployment_status(
    State(state): State<AppState>,
    Path(program_name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    match deployment_manager.get_deployment_status(&program_name).await {
        Some(status) => Ok(Json(status)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Get all deployments
pub async fn get_all_deployments(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let deployments = deployment_manager.get_all_deployments().await;
    Ok(Json(deployments))
}

/// Verify program deployment
pub async fn verify_deployment(
    State(state): State<AppState>,
    Path(program_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let program_pubkey = Pubkey::from_str(&program_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    match deployment_manager.verify_deployment(&program_pubkey).await {
        Ok(is_deployed) => {
            let rpc_service = state.solana_rpc_service.as_ref()
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            
            let account_info = rpc_service.get_account(&program_pubkey).await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let response = VerificationResponse {
                program_id,
                is_deployed,
                is_executable: account_info.as_ref().map(|a| a.executable).unwrap_or(false),
                account_exists: account_info.is_some(),
            };
            
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to verify deployment: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get deployment manager status
pub async fn get_deployment_manager_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let deployment_manager = state.solana_deployment_manager.as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    let status = deployment_manager.get_status().await;
    Ok(Json(status))
}

/// Program IDL (Interface Definition Language) endpoints
#[derive(Debug, Serialize)]
pub struct ProgramIdl {
    pub version: String,
    pub name: String,
    pub instructions: Vec<IdlInstruction>,
    pub accounts: Vec<IdlAccount>,
    pub types: Vec<IdlType>,
    pub events: Vec<IdlEvent>,
    pub errors: Vec<IdlError>,
}

#[derive(Debug, Serialize)]
pub struct IdlInstruction {
    pub name: String,
    pub accounts: Vec<IdlAccountItem>,
    pub args: Vec<IdlField>,
}

#[derive(Debug, Serialize)]
pub struct IdlAccountItem {
    pub name: String,
    pub is_mut: bool,
    pub is_signer: bool,
}

#[derive(Debug, Serialize)]
pub struct IdlAccount {
    pub name: String,
    pub type_def: IdlTypeDefinition,
}

#[derive(Debug, Serialize)]
pub struct IdlType {
    pub name: String,
    pub type_def: IdlTypeDefinition,
}

#[derive(Debug, Serialize)]
pub struct IdlTypeDefinition {
    pub kind: String,
    pub fields: Vec<IdlField>,
}

#[derive(Debug, Serialize)]
pub struct IdlField {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Serialize)]
pub struct IdlEvent {
    pub name: String,
    pub fields: Vec<IdlField>,
}

#[derive(Debug, Serialize)]
pub struct IdlError {
    pub code: u32,
    pub name: String,
    pub msg: String,
}

/// Get program IDL
pub async fn get_program_idl(
    State(state): State<AppState>,
    Path(program_name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // For the betting platform, return a hardcoded IDL
    // In production, this would be fetched from the deployed program or a registry
    
    if program_name == "betting_platform" {
        let idl = ProgramIdl {
            version: "0.1.0".to_string(),
            name: "betting_platform".to_string(),
            instructions: vec![
                IdlInstruction {
                    name: "create_market".to_string(),
                    accounts: vec![
                        IdlAccountItem {
                            name: "market".to_string(),
                            is_mut: true,
                            is_signer: false,
                        },
                        IdlAccountItem {
                            name: "creator".to_string(),
                            is_mut: true,
                            is_signer: true,
                        },
                        IdlAccountItem {
                            name: "system_program".to_string(),
                            is_mut: false,
                            is_signer: false,
                        },
                    ],
                    args: vec![
                        IdlField {
                            name: "market_id".to_string(),
                            type_name: "u128".to_string(),
                        },
                        IdlField {
                            name: "title".to_string(),
                            type_name: "String".to_string(),
                        },
                        IdlField {
                            name: "description".to_string(),
                            type_name: "String".to_string(),
                        },
                        IdlField {
                            name: "outcomes".to_string(),
                            type_name: "Vec<String>".to_string(),
                        },
                        IdlField {
                            name: "end_time".to_string(),
                            type_name: "i64".to_string(),
                        },
                        IdlField {
                            name: "creator_fee_bps".to_string(),
                            type_name: "u16".to_string(),
                        },
                    ],
                },
                IdlInstruction {
                    name: "place_trade".to_string(),
                    accounts: vec![
                        IdlAccountItem {
                            name: "market".to_string(),
                            is_mut: true,
                            is_signer: false,
                        },
                        IdlAccountItem {
                            name: "position".to_string(),
                            is_mut: true,
                            is_signer: false,
                        },
                        IdlAccountItem {
                            name: "trader".to_string(),
                            is_mut: true,
                            is_signer: true,
                        },
                        IdlAccountItem {
                            name: "demo_account".to_string(),
                            is_mut: true,
                            is_signer: false,
                        },
                    ],
                    args: vec![
                        IdlField {
                            name: "market_id".to_string(),
                            type_name: "u128".to_string(),
                        },
                        IdlField {
                            name: "outcome".to_string(),
                            type_name: "u8".to_string(),
                        },
                        IdlField {
                            name: "amount".to_string(),
                            type_name: "u64".to_string(),
                        },
                        IdlField {
                            name: "side".to_string(),
                            type_name: "u8".to_string(),
                        },
                        IdlField {
                            name: "leverage".to_string(),
                            type_name: "u32".to_string(),
                        },
                    ],
                },
            ],
            accounts: vec![
                IdlAccount {
                    name: "Market".to_string(),
                    type_def: IdlTypeDefinition {
                        kind: "struct".to_string(),
                        fields: vec![
                            IdlField {
                                name: "id".to_string(),
                                type_name: "u128".to_string(),
                            },
                            IdlField {
                                name: "title".to_string(),
                                type_name: "String".to_string(),
                            },
                            IdlField {
                                name: "resolved".to_string(),
                                type_name: "bool".to_string(),
                            },
                            IdlField {
                                name: "winning_outcome".to_string(),
                                type_name: "Option<u8>".to_string(),
                            },
                        ],
                    },
                },
                IdlAccount {
                    name: "Position".to_string(),
                    type_def: IdlTypeDefinition {
                        kind: "struct".to_string(),
                        fields: vec![
                            IdlField {
                                name: "id".to_string(),
                                type_name: "u128".to_string(),
                            },
                            IdlField {
                                name: "market_id".to_string(),
                                type_name: "u128".to_string(),
                            },
                            IdlField {
                                name: "owner".to_string(),
                                type_name: "Pubkey".to_string(),
                            },
                            IdlField {
                                name: "amount".to_string(),
                                type_name: "u64".to_string(),
                            },
                        ],
                    },
                },
            ],
            types: vec![],
            events: vec![
                IdlEvent {
                    name: "MarketCreated".to_string(),
                    fields: vec![
                        IdlField {
                            name: "market_id".to_string(),
                            type_name: "u128".to_string(),
                        },
                        IdlField {
                            name: "creator".to_string(),
                            type_name: "Pubkey".to_string(),
                        },
                    ],
                },
                IdlEvent {
                    name: "TradeExecuted".to_string(),
                    fields: vec![
                        IdlField {
                            name: "market_id".to_string(),
                            type_name: "u128".to_string(),
                        },
                        IdlField {
                            name: "trader".to_string(),
                            type_name: "Pubkey".to_string(),
                        },
                        IdlField {
                            name: "amount".to_string(),
                            type_name: "u64".to_string(),
                        },
                    ],
                },
            ],
            errors: vec![
                IdlError {
                    code: 6000,
                    name: "MarketNotFound".to_string(),
                    msg: "The specified market does not exist".to_string(),
                },
                IdlError {
                    code: 6001,
                    name: "MarketResolved".to_string(),
                    msg: "Market has already been resolved".to_string(),
                },
                IdlError {
                    code: 6002,
                    name: "InsufficientFunds".to_string(),
                    msg: "Insufficient funds for this operation".to_string(),
                },
            ],
        };
        
        Ok(Json(idl))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}