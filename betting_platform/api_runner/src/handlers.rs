//! Request handlers for the betting platform API

pub mod price_feed;
pub mod wallet_http;
pub mod settlement;
pub mod db_handlers;
pub mod cache_handlers;
pub mod queue_handlers;
pub mod quantum_settlement_handlers;
pub mod polymarket_orders;

use anyhow::Result;
use std::str::FromStr;
use axum::{
    extract::{Path, State, Query},
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use serde_json::json;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
};
use tracing::{info, debug, error};
use crate::{AppState, types::*, verse_generator::VerseGenerator, wallet_utils::WalletType, validation::ValidatedJson, response::{ApiResponse, responses}, queue};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

/// Get program information
pub async fn get_program_info(State(state): State<AppState>) -> Response {
    match state.platform_client.get_program_state().await {
        Ok(program_state) => {
            responses::ok(json!({
                "program_id": state.program_id.to_string(),
                "admin": program_state.admin.to_string(),
                "total_markets": program_state.total_markets,
                "total_volume": program_state.total_volume,
                "protocol_fee_rate": program_state.protocol_fee_rate,
                "min_bet_amount": program_state.min_bet_amount,
                "max_bet_amount": program_state.max_bet_amount,
                "emergency_mode": program_state.emergency_mode,
            })).into_response()
        }
        Err(e) => {
            responses::internal_error(format!("Failed to get program state: {}", e)).into_response()
        }
    }
}

// Module integration_simple is now in a separate file
pub mod integration_simple;

/// Proxy endpoint for Polymarket markets
pub async fn proxy_polymarket_markets(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let limit = params.get("limit").unwrap_or(&"10".to_string()).clone();
    let search = params.get("search");

    let base_url = env::var("POLYMARKET_CLOB_BASE_URL")
        .unwrap_or_else(|_| "https://clob.polymarket.com".to_string())
        .trim_end_matches('/')
        .to_string();
    
    // Make request to Polymarket API
    let url = if let Some(search_term) = search {
        format!("{}/markets?limit={}&search={}", base_url, limit, search_term)
    } else {
        format!("{}/markets?limit={}", base_url, limit)
    };
    
    match reqwest::get(&url).await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(json_data) => {
                    // Handle both direct array and wrapped response with possible null data
                    let data = if json_data.is_array() {
                        json_data.as_array().unwrap().clone()
                    } else if let Some(data_field) = json_data.get("data") {
                        if data_field.is_null() {
                            Vec::new()
                        } else if let Some(arr) = data_field.as_array() {
                            arr.clone()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };
                    
                    // Generate verses for each market
                    let mut verse_gen = VerseGenerator::new();
                    let mut enhanced_data = Vec::new();
                    
                    for market in data.iter() {
                        if let Some(market_obj) = market.as_object() {
                            let mut enhanced_market = market_obj.clone();
                            // Ensure required fields exist
                            if !enhanced_market.contains_key("id") {
                                enhanced_market.insert("id".to_string(), json!(uuid::Uuid::new_v4().to_string()));
                            }
                            if !enhanced_market.contains_key("title") && enhanced_market.contains_key("question") {
                                if let Some(question) = enhanced_market.get("question") {
                                    enhanced_market.insert("title".to_string(), question.clone());
                                }
                            }
                            if !enhanced_market.contains_key("liquidity") {
                                enhanced_market.insert("liquidity".to_string(), json!("0"));
                            }
                            
                            // Generate verses for this market
                            let verses = verse_gen.generate_verses_for_market(&json!({
                                "title": enhanced_market.get("title").cloned().unwrap_or(json!("")),
                                "question": enhanced_market.get("question").cloned().unwrap_or(json!("")),
                                "tags": enhanced_market.get("tags").cloned().unwrap_or(json!([])),
                                "category": enhanced_market.get("category").cloned().unwrap_or(json!("General")),
                                "id": enhanced_market.get("id").cloned().unwrap_or(json!(""))
                            }));
                            enhanced_market.insert("verses".to_string(), json!(verses));
                            enhanced_data.push(json!(enhanced_market));
                        }
                    }
                    
                    Json(enhanced_data).into_response()
                }
                Err(e) => {
                    (StatusCode::BAD_GATEWAY, Json(json!({
                        "error": {
                            "code": "POLYMARKET_PARSE_ERROR",
                            "message": format!("Failed to parse Polymarket response: {}", e),
                        }
                    }))).into_response()
                }
            }
        }
        Err(e) => {
            (StatusCode::BAD_GATEWAY, Json(json!({
                "error": {
                    "code": "POLYMARKET_API_ERROR",
                    "message": format!("Failed to fetch from Polymarket: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Market query parameters
#[derive(Debug, serde::Deserialize)]
pub struct MarketsQuery {
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort: Option<String>, // Alternative for sort_by
    #[serde(default)]
    pub status: Option<String>,
}

/// Get all markets with caching
pub async fn get_markets(
    Query(params): Query<MarketsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let cache_key = crate::cache::CacheKey::markets_list();
    
    // Try cache first
    if let Some(cached_markets) = state.cache.get::<serde_json::Value>(&cache_key).await {
        tracing::debug!("Returning cached markets");
        return Json(cached_markets).into_response();
    }
    
    // Cache miss - fetch from source  
    tracing::info!("Cache miss, fetching markets from source");
    
    // Try database first if available
    let mut markets = match state.database.get_connection().await {
            Ok(conn) => {
                let limit = params.limit.unwrap_or(10).min(100) as i64;
                let offset = params.offset.unwrap_or(0) as i64;
                
                match crate::db::market_queries::get_all_markets(&conn, limit, offset).await {
                    Ok(db_markets) => {
                        tracing::info!("Successfully fetched {} markets from database", db_markets.len());
                        // Convert database markets to API format
                        db_markets.into_iter().map(|m| Market {
                            id: m.id as u128,
                            title: m.question.clone(),
                            description: m.description.unwrap_or_default(),
                            outcomes: m.outcomes.as_array()
                                .map(|arr| arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .enumerate()
                                    .map(|(i, name)| MarketOutcome {
                                        id: i as u8,
                                        name: name.to_string(),
                                        title: name.to_string(),
                                        description: format!("{} outcome", name),
                                        total_stake: 0,
                                    })
                                    .collect())
                                .unwrap_or_else(|| vec![
                                    MarketOutcome { id: 0, name: "Yes".to_string(), title: "Yes".to_string(), description: "Yes outcome".to_string(), total_stake: 0 },
                                    MarketOutcome { id: 1, name: "No".to_string(), title: "No".to_string(), description: "No outcome".to_string(), total_stake: 0 },
                                ]),
                            creator: Pubkey::new_unique(),
                            total_liquidity: m.total_liquidity as u64,
                            total_volume: m.total_volume as u64,
                            resolution_time: m.end_time.timestamp(),
                            resolved: m.status == "resolved",
                            winning_outcome: m.resolution_outcome.map(|o| o as u8),
                            amm_type: match m.market_type.as_str() {
                                "cpmm" => AmmType::Cpmm,
                                "lmsr" => AmmType::Lmsr,
                                "pmamm" => AmmType::PmAmm,
                                "l2amm" => AmmType::L2Amm,
                                "hybrid" => AmmType::Hybrid,
                                _ => AmmType::Cpmm,
                            },
                            created_at: m.created_at.timestamp(),
                            verse_id: None,
                            current_price: 0.5, // Default price
                        }).collect()
                    },
                    Err(e) => {
                        tracing::warn!("Failed to fetch from database, trying Polymarket: {}", e);
                        Vec::new()
                    }
                }
            },
            Err(e) => {
                tracing::warn!("Failed to get database connection: {}", e);
                Vec::new()
            }
        };
    
    // If no markets from database, try Polymarket
    if markets.is_empty() {
        markets = match fetch_polymarket_markets(&state, 100).await {
            Ok(polymarket_markets) => {
                tracing::info!("Successfully fetched {} Polymarket markets", polymarket_markets.len());
                polymarket_markets
            },
            Err(e) => {
                tracing::error!("Failed to fetch Polymarket data: {}", e);
                
                // Last resort: use seeded markets
                let seeded_markets = state.seeded_markets.get_all();
                tracing::warn!("Using {} seeded markets as fallback", seeded_markets.len());
                seeded_markets
            }
        };
    }
    
    // Apply search filter if provided
    if let Some(search) = &params.search {
        let search_lower = search.to_lowercase();
        markets.retain(|m| {
            m.title.to_lowercase().contains(&search_lower) ||
            m.description.to_lowercase().contains(&search_lower)
        });
    }
    
    // Apply status filter if provided
    if let Some(status) = &params.status {
        match status.as_str() {
            "active" => markets.retain(|m| !m.resolved && m.resolution_time > chrono::Utc::now().timestamp()),
            "resolved" => markets.retain(|m| m.resolved),
            _ => {}
        }
    }
    
    // Apply sorting (check both sort and sort_by parameters)
    let sort_param = params.sort.as_ref().or(params.sort_by.as_ref());
    if let Some(sort_by) = sort_param {
        match sort_by.as_str() {
            "volume" => markets.sort_by(|a, b| b.total_volume.cmp(&a.total_volume)),
            "liquidity" => markets.sort_by(|a, b| b.total_liquidity.cmp(&a.total_liquidity)),
            "created" => markets.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            _ => {}
        }
    }
    
    let total_count = markets.len();
    
    // Apply pagination
    let limit = params.limit.unwrap_or(10).min(100);
    let offset = params.offset.unwrap_or(0);
    
    let source = if markets.iter().any(|m| m.id >= 1000) { "polymarket_live" } else { "seeded_data" };
    
    let paginated_markets: Vec<_> = markets.into_iter()
        .skip(offset)
        .take(limit)
        .collect();
    
    tracing::info!("Returning {} markets (total: {})", paginated_markets.len(), total_count);
    
    let response = crate::response_types::MarketsResponse::new(
        paginated_markets,
        total_count,
        limit,
        offset,
        source
    );
    
    // Cache the response for 5 minutes
    if let Err(e) = state.cache.set(&cache_key, &response, Some(300)).await {
        tracing::warn!("Failed to cache markets: {}", e);
    }
    
    crate::throughput_optimization::FastJson(response).into_response()
}

/// Get a specific market
pub async fn get_market(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let market_id = match id.parse::<u128>() {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_MARKET_ID",
                    "message": "Market ID must be a valid number",
                }
            }))).into_response()
        }
    };
    
    // Check if this is a Polymarket ID (offset by 1000)
    if market_id >= 1000 && market_id < 2000 {
        // Fetch Polymarket markets and find the one with this ID
        match fetch_polymarket_markets(&state, 100).await {
            Ok(markets) => {
                if let Some(market) = markets.into_iter().find(|m| m.id == market_id) {
                    return Json(json!({
                        "id": market.id,
                        "title": market.title,
                        "description": market.description,
                        "outcomes": market.outcomes,
                        "creator": market.creator.to_string(),
                        "total_liquidity": market.total_liquidity,
                        "total_volume": market.total_volume,
                        "resolution_time": market.resolution_time,
                        "resolved": market.resolved,
                        "winning_outcome": market.winning_outcome,
                        "amm_type": market.amm_type,
                        "created_at": market.created_at,
                        "verse_id": market.verse_id,
                        "source": "polymarket",
                    })).into_response();
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch Polymarket market {}: {}", market_id, e);
            }
        }
    }
    
    // If not found in Polymarket, check if it's a blockchain market ID
    
    // Fall back to on-chain market
    match state.platform_client.get_market(market_id).await {
        Ok(Some(market)) => Json(json!({
            "id": market.id,
            "title": market.title,
            "description": market.description,
            "outcomes": market.outcomes,
            "creator": market.creator.to_string(),
            "total_liquidity": market.total_liquidity,
            "total_volume": market.total_volume,
            "resolution_time": market.resolution_time,
            "resolved": market.resolved,
            "winning_outcome": market.winning_outcome,
            "amm_type": market.amm_type,
            "created_at": market.created_at,
            "verse_id": market.verse_id,
            "source": "blockchain",
        })).into_response(),
        Ok(None) => {
            (StatusCode::NOT_FOUND, Json(json!({
                "error": {
                    "code": "MARKET_NOT_FOUND",
                    "message": "Market not found",
                }
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "MARKET_FETCH_ERROR",
                    "message": format!("Failed to fetch market: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Create a new market
pub async fn create_market(
    State(state): State<AppState>,
    Json(payload): Json<CreateMarketRequest>,
) -> impl IntoResponse {
    
    // Create market account
    let market_keypair = Keypair::new();
    let admin_keypair = Keypair::new(); // In production, this would come from the user
    
    match state.platform_client.create_market(
        &admin_keypair,
        &market_keypair.pubkey(),
        &payload.question,
        &payload.outcomes,
        payload.end_time,
        MarketType::Binary, // Default to Binary for now
        250, // 2.5% default fee rate
    ).await {
        Ok(signature) => {
            // Emit real-time event for market creation
            if let Some(queue) = &state.queue_service {
                let msg = queue::QueueMessage::MarketCreated {
                    market_id: market_keypair.pubkey().to_string(),
                    title: payload.question.clone(),
                    creator: admin_keypair.pubkey().to_string(),
                    timestamp: chrono::Utc::now(),
                };
                let _ = queue.publish(queue::QueueChannels::MARKETS, msg).await;
            }
            
            responses::created(json!({
                "market_id": market_keypair.pubkey().to_string(),
                "signature": signature,
                "message": "Market created successfully",
            }))
        }
        Err(e) => {
            responses::internal_error(format!("Failed to create market: {}", e)).into_response()
        }
    }
}

/// Get market orderbook
pub async fn get_market_orderbook(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let market_id = match id.parse::<u128>() {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_MARKET_ID",
                    "message": "Market ID must be a valid number",
                }
            }))).into_response()
        }
    };
    
    match state.platform_client.get_market_orderbook(market_id).await {
        Ok(orderbook) => Json(orderbook).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "ORDERBOOK_FETCH_ERROR",
                    "message": format!("Failed to fetch orderbook: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Place a trade
pub async fn place_trade(
    State(state): State<AppState>,
    Json(payload): Json<PlaceTradeRequest>,
) -> impl IntoResponse {
    // Input is already validated by ValidatedJson
    if payload.amount == 0 {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": {
                "code": "INVALID_AMOUNT",
                "message": "Amount must be greater than 0",
            }
        }))).into_response();
    }
    
    // Check if wallet is demo
    let wallet_type = match WalletType::from_string(&payload.wallet) {
        Ok(wt) => wt,
        Err(_) => {
            // If wallet parsing fails, continue with regular flow
            WalletType::Real(Keypair::new().pubkey())
        }
    };
    
    // For demo wallets, return mock success
    if wallet_type.is_demo() {
        let mock_signature = format!("demo_sig_{}", uuid::Uuid::new_v4());
        
        // Emit real-time event for demo trade
        if let Some(queue) = &state.queue_service {
            let msg = queue::QueueMessage::TradeExecuted {
                trade_id: mock_signature.clone(),
                wallet: payload.wallet.clone(),
                market_id: payload.market_id.to_string(),
                amount: payload.amount,
                outcome: payload.outcome,
                timestamp: chrono::Utc::now(),
            };
            let _ = queue.publish(queue::QueueChannels::TRADES, msg).await;
        }
        
        // Broadcast trade update
        state.ws_manager.broadcast(WsMessage::Notification {
            title: "Demo Trade Placed".to_string(),
            message: format!("Demo trade placed on market {} for {} with {}x leverage", 
                payload.market_id, payload.amount, payload.leverage.unwrap_or(1)),
            level: "info".to_string(),
        });
        
        return Json(json!({
            "signature": mock_signature,
            "trader": payload.wallet,
            "message": "Demo trade placed successfully",
            "is_demo": true,
            "position_id": uuid::Uuid::new_v4().to_string(),
            "amount": payload.amount,
            "leverage": payload.leverage.unwrap_or(1),
        })).into_response();
    }
    
    // Create trader account (in production, this would come from the user's wallet)
    let trader_keypair = Keypair::new();
    
    match state.platform_client.place_bet(
        &trader_keypair,
        payload.market_id,
        payload.outcome,
        payload.amount,
        payload.leverage.unwrap_or(1),
        payload.order_type.unwrap_or(OrderType::Market),
    ).await {
        Ok(signature) => {
            // Emit real-time event for actual trade
            if let Some(queue) = &state.queue_service {
                let msg = queue::QueueMessage::TradeExecuted {
                    trade_id: signature.clone(),
                    wallet: trader_keypair.pubkey().to_string(),
                    market_id: payload.market_id.to_string(),
                    amount: payload.amount,
                    outcome: payload.outcome,
                    timestamp: chrono::Utc::now(),
                };
                let _ = queue.publish(queue::QueueChannels::TRADES, msg).await;
            }
            
            // Broadcast trade update
            let update = json!({
                "type": "trade_placed",
                "market_id": payload.market_id,
                "outcome": payload.outcome,
                "amount": payload.amount,
                "leverage": payload.leverage.unwrap_or(1),
                "trader": trader_keypair.pubkey().to_string(),
                "signature": signature,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });
            
            state.ws_manager.broadcast(WsMessage::Notification {
                title: "Trade Placed".to_string(),
                message: format!("Trade placed on market {} for {} with {}x leverage", payload.market_id, payload.amount, payload.leverage.unwrap_or(1)),
                level: "info".to_string(),
            });
            
            Json(json!({
                "signature": signature,
                "trader": trader_keypair.pubkey().to_string(),
                "message": "Trade placed successfully",
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "TRADE_PLACE_ERROR",
                    "message": format!("Failed to place trade: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Place a funded trade (automatically handles account funding)
pub async fn place_funded_trade(
    State(state): State<AppState>,
    Json(payload): Json<PlaceTradeRequest>,
) -> impl IntoResponse {
    // Check if funded trading is enabled
    let funded_client = match &state.funded_trading_client {
        Some(client) => client,
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
                "error": {
                    "code": "FUNDED_TRADING_DISABLED",
                    "message": "Funded trading is not enabled. Set ENABLE_AUTO_FUNDING=true",
                }
            }))).into_response();
        }
    };

    // Validate input
    if payload.amount == 0 {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": {
                "code": "INVALID_AMOUNT",
                "message": "Amount must be greater than 0",
            }
        }))).into_response();
    }

    // Create or get funded wallet
    let (wallet, funding_signature) = match funded_client.create_funded_demo_account().await {
        Ok((keypair, sig)) => (keypair, sig),
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "FUNDING_ERROR",
                    "message": format!("Failed to create funded account: {}", e),
                }
            }))).into_response();
        }
    };

    // Get account status to confirm funding
    let account_status = match funded_client.get_account_status(&wallet.pubkey()).await {
        Ok(status) => status,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "ACCOUNT_STATUS_ERROR",
                    "message": format!("Failed to check account status: {}", e),
                }
            }))).into_response();
        }
    };

    // Place the funded trade
    match funded_client.place_trade_with_funding(
        &wallet,
        payload.market_id,
        payload.amount,
        payload.outcome,
        payload.leverage.unwrap_or(1),
    ).await {
        Ok(trade_signature) => {
            // Broadcast trade update
            state.ws_manager.broadcast(WsMessage::Notification {
                title: "Funded Trade Placed".to_string(),
                message: format!("Funded trade placed on market {} for {} with {}x leverage", 
                               payload.market_id, payload.amount, payload.leverage.unwrap_or(1)),
                level: "success".to_string(),
            });

            Json(json!({
                "trade_signature": trade_signature,
                "funding_signature": funding_signature,
                "wallet": wallet.pubkey().to_string(),
                "account_status": account_status,
                "market_id": payload.market_id,
                "amount": payload.amount,
                "outcome": payload.outcome,
                "leverage": payload.leverage.unwrap_or(1),
                "status": "success",
                "message": "Funded trade placed successfully",
                "auto_funded": true,
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "FUNDED_TRADE_ERROR",
                    "message": format!("Failed to place funded trade: {}", e),
                },
                "funding_info": {
                    "wallet": wallet.pubkey().to_string(),
                    "funding_signature": funding_signature,
                    "account_status": account_status,
                }
            }))).into_response()
        }
    }
}

/// Close a position
pub async fn close_position(
    State(state): State<AppState>,
    Json(payload): Json<ClosePositionRequest>,
) -> impl IntoResponse {
    let trader_keypair = Keypair::new(); // In production, from user's wallet
    
    match state.platform_client.close_position(
        &trader_keypair,
        &Pubkey::from_str(&payload.position_id).unwrap(),
    ).await {
        Ok(signature) => {
            // Emit real-time event for position closure
            if let Some(queue) = &state.queue_service {
                let msg = queue::QueueMessage::PositionClosed {
                    position_id: payload.position_id.clone(),
                    wallet: trader_keypair.pubkey().to_string(),
                    market_id: "0".to_string(), // Market ID would need to be retrieved from position
                    pnl: 0, // Would need to calculate actual P&L
                    timestamp: chrono::Utc::now(),
                };
                let _ = queue.publish(queue::QueueChannels::TRADES, msg).await;
            }
            
            Json(json!({
                "signature": signature,
                "message": "Position closed successfully",
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "POSITION_CLOSE_ERROR",
                    "message": format!("Failed to close position: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Get positions query parameters
#[derive(Debug, serde::Deserialize)]
pub struct GetPositionsQuery {
    wallet: String,
}

/// Get positions by query parameter
pub async fn get_positions_query(
    Query(params): Query<GetPositionsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    get_positions(Path(params.wallet), State(state)).await
}

/// Get positions for a wallet
pub async fn get_positions(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let wallet_type = match WalletType::from_string(&wallet) {
        Ok(wt) => wt,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_WALLET",
                    "message": e,
                }
            }))).into_response()
        }
    };
    
    // For demo wallets, return empty positions (or mock data)
    if wallet_type.is_demo() {
        return Json(json!({
            "positions": [],
            "is_demo": true,
        })).into_response();
    }
    
    let wallet_pubkey = wallet_type.as_pubkey();
    match state.platform_client.get_user_positions(&wallet_pubkey).await {
        Ok(positions) => Json(json!({
            "positions": positions,
            "is_demo": false,
        })).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "POSITIONS_FETCH_ERROR",
                    "message": format!("Failed to fetch positions: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Get wallet balance
pub async fn get_balance(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let wallet_type = match WalletType::from_string(&wallet) {
        Ok(wt) => wt,
        Err(e) => {
            return responses::bad_request(e).into_response()
        }
    };
    
    // For demo wallets, return a mock balance
    if wallet_type.is_demo() {
        return responses::ok(json!({
            "wallet": wallet,
            "balance": 1_000_000_000, // 1 SOL for demo accounts
            "sol": 1.0,
            "is_demo": true,
        })).into_response();
    }
    
    // For real wallets, check cache first
    let cache_key = crate::cache::CacheKey::wallet_balance(&wallet);
    if let Some(cached_balance) = state.cache.get::<serde_json::Value>(&cache_key).await {
        tracing::debug!("Returning cached balance for wallet: {}", wallet);
        return Json(cached_balance).into_response();
    }
    
    // Cache miss - fetch from blockchain
    let wallet_pubkey = wallet_type.as_pubkey();
    match state.rpc_client.get_balance(&wallet_pubkey) {
        Ok(balance) => {
            let response = json!({
                "wallet": wallet,
                "balance": balance,
                "sol": balance as f64 / 1_000_000_000.0,
                "is_demo": false,
                "cached": false
            });
            
            // Cache balance for 30 seconds (balances change frequently)
            if let Err(e) = state.cache.set(&cache_key, &response, Some(30)).await {
                tracing::warn!("Failed to cache balance: {}", e);
            }
            
            Json(response).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "BALANCE_FETCH_ERROR",
                    "message": format!("Failed to fetch balance: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Create a demo account
pub async fn create_demo_account(
    State(state): State<AppState>,
    Json(payload): Json<CreateDemoAccountRequest>,
) -> impl IntoResponse {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    
    // In a real implementation, we would:
    // 1. Create the account on-chain
    // 2. Fund it with demo tokens
    // 3. Store the keypair securely
    
    // For now, return the account details
    let initial_balance = payload.initial_balance.unwrap_or(10000);
    
    Json(json!({
        "wallet_address": pubkey.to_string(),
        "private_key": bs58::encode(keypair.to_bytes()).into_string(),
        "balance": initial_balance,
        "message": "Demo account created successfully",
        "warning": "This is a demo account. Do not send real funds to this address.",
    })).into_response()
}

/// Get portfolio information
pub async fn get_portfolio(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let wallet_type = match WalletType::from_string(&wallet) {
        Ok(wt) => wt,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_WALLET",
                    "message": e,
                }
            }))).into_response()
        }
    };
    
    // For demo wallets, return mock portfolio data
    if wallet_type.is_demo() {
        return Json(json!({
            "wallet": wallet,
            "balance": {
                "sol": 1.0,
                "lamports": 1_000_000_000,
            },
            "positions": {
                "total": 0,
                "open": 0,
                "closed": 0,
            },
            "pnl": {
                "total": 0,
                "formatted": "0.00",
            },
            "positions_list": [],
            "is_demo": true,
        })).into_response();
    }
    
    // For real wallets, get actual data
    let wallet_pubkey = wallet_type.as_pubkey();
    
    // Get balance
    let balance = match state.rpc_client.get_balance(&wallet_pubkey) {
        Ok(b) => b,
        Err(_) => 0,
    };
    
    // Get positions
    let positions = match state.platform_client.get_user_positions(&wallet_pubkey).await {
        Ok(p) => p,
        Err(_) => vec![],
    };
    
    // Calculate portfolio metrics
    let total_positions = positions.len();
    let open_positions = positions.iter().filter(|p| p.status == PositionStatus::Open).count();
    let total_pnl: i128 = positions.iter().map(|p| p.pnl).sum();
    
    Json(json!({
        "wallet": wallet,
        "balance": {
            "sol": balance as f64 / 1_000_000_000.0,
            "lamports": balance,
        },
        "positions": {
            "total": total_positions,
            "open": open_positions,
            "closed": total_positions - open_positions,
        },
        "pnl": {
            "total": total_pnl,
            "formatted": format!("{:.2}", total_pnl as f64 / 1_000_000.0),
        },
        "positions_list": positions,
        "is_demo": false,
    })).into_response()
}

/// Get risk metrics for a wallet
pub async fn get_risk_metrics(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let wallet_type = match WalletType::from_string(&wallet) {
        Ok(wt) => wt,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_WALLET",
                    "message": e,
                }
            }))).into_response()
        }
    };
    
    // For demo wallets, return mock risk metrics
    if wallet_type.is_demo() {
        return Json(json!({
            "wallet": wallet,
            "risk_metrics": {
                "risk_score": 25.0,
                "leverage_ratio": 2.0,
                "var_95": 0.05,
                "margin_ratio": 0.4,
                "win_rate": 0.5,
                "sharpe_ratio": 1.2,
                "portfolio_value": 1000000,
            },
            "is_demo": true,
        })).into_response();
    }
    
    let wallet_pubkey = wallet_type.as_pubkey();
    
    // Get positions
    let positions = match state.platform_client.get_user_positions(&wallet_pubkey).await {
        Ok(p) => p,
        Err(_) => vec![],
    };
    
    // Calculate risk metrics
    let open_positions: Vec<_> = positions.iter().filter(|p| p.status == PositionStatus::Open).collect();
    let total_exposure: u64 = open_positions.iter().map(|p| p.amount).sum();
    let total_leverage: u32 = open_positions.iter().map(|p| p.leverage).sum();
    let avg_leverage = if !open_positions.is_empty() {
        total_leverage as f64 / open_positions.len() as f64
    } else {
        0.0
    };
    
    // Calculate maximum drawdown
    let pnl_values: Vec<i128> = positions.iter().map(|p| p.pnl).collect();
    let mut max_drawdown = 0i128;
    let mut peak = 0i128;
    let mut running_total = 0i128;
    
    for pnl in pnl_values {
        running_total += pnl;
        if running_total > peak {
            peak = running_total;
        }
        let drawdown = peak - running_total;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    
    // Calculate win rate
    let winning_positions = positions.iter().filter(|p| p.pnl > 0).count();
    let losing_positions = positions.iter().filter(|p| p.pnl < 0).count();
    let total_closed = winning_positions + losing_positions;
    let win_rate = if total_closed > 0 {
        (winning_positions as f64 / total_closed as f64) * 100.0
    } else {
        0.0
    };
    
    // Calculate Sharpe ratio (simplified)
    let returns: Vec<f64> = positions.iter()
        .filter(|p| p.status == PositionStatus::Closed)
        .map(|p| p.pnl as f64 / p.amount as f64)
        .collect();
    
    let avg_return = if !returns.is_empty() {
        returns.iter().sum::<f64>() / returns.len() as f64
    } else {
        0.0
    };
    
    let variance = if returns.len() > 1 {
        returns.iter()
            .map(|r| (r - avg_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64
    } else {
        0.0
    };
    
    let std_dev = variance.sqrt();
    let sharpe_ratio = if std_dev > 0.0 {
        avg_return / std_dev * (252.0_f64).sqrt() // Annualized
    } else {
        0.0
    };
    
    // Risk score (0-100)
    let risk_score = calculate_risk_score(avg_leverage, max_drawdown, win_rate, sharpe_ratio);
    
    Json(json!({
        "wallet": wallet,
        "exposure": {
            "total": total_exposure,
            "open_positions": open_positions.len(),
            "average_leverage": avg_leverage,
        },
        "performance": {
            "win_rate": win_rate,
            "winning_positions": winning_positions,
            "losing_positions": losing_positions,
            "max_drawdown": max_drawdown,
            "sharpe_ratio": sharpe_ratio,
        },
        "risk_score": risk_score,
        "recommendations": get_risk_recommendations(risk_score, avg_leverage, win_rate),
    })).into_response()
}

fn calculate_risk_score(avg_leverage: f64, max_drawdown: i128, win_rate: f64, sharpe_ratio: f64) -> u8 {
    let mut score: f64 = 50.0;
    
    // Leverage impact (-20 to +10)
    if avg_leverage > 10.0 {
        score -= 20.0;
    } else if avg_leverage > 5.0 {
        score -= 10.0;
    } else if avg_leverage < 2.0 {
        score += 10.0;
    }
    
    // Drawdown impact (-20 to +10)
    let drawdown_percent = max_drawdown as f64 / 1_000_000.0; // Assuming base unit
    if drawdown_percent > 50.0 {
        score -= 20.0;
    } else if drawdown_percent > 25.0 {
        score -= 10.0;
    } else if drawdown_percent < 10.0 {
        score += 10.0;
    }
    
    // Win rate impact (-10 to +20)
    if win_rate > 60.0 {
        score += 20.0;
    } else if win_rate > 50.0 {
        score += 10.0;
    } else if win_rate < 40.0 {
        score -= 10.0;
    }
    
    // Sharpe ratio impact (-10 to +20)
    if sharpe_ratio > 2.0 {
        score += 20.0;
    } else if sharpe_ratio > 1.0 {
        score += 10.0;
    } else if sharpe_ratio < 0.0 {
        score -= 10.0;
    }
    
    score.max(0.0).min(100.0) as u8
}

fn get_risk_recommendations(risk_score: u8, avg_leverage: f64, win_rate: f64) -> Vec<String> {
    let mut recommendations = Vec::new();
    
    if risk_score < 30 {
        recommendations.push("⚠️ High risk detected. Consider reducing position sizes.".to_string());
    }
    
    if avg_leverage > 5.0 {
        recommendations.push("📊 Your average leverage is high. Consider using lower leverage to reduce risk.".to_string());
    }
    
    if win_rate < 45.0 {
        recommendations.push("📈 Your win rate is below average. Review your trading strategy.".to_string());
    }
    
    if risk_score > 70 {
        recommendations.push("✅ Good risk management! Keep up the disciplined approach.".to_string());
    }
    
    if recommendations.is_empty() {
        recommendations.push("📊 Continue monitoring your positions and maintain proper risk management.".to_string());
    }
    
    recommendations
}

/// Get verses with caching
pub async fn get_verses(State(state): State<AppState>) -> impl IntoResponse {
    let cache_key = crate::cache::CacheKey::verses_list();
    
    // Try cache first
    if let Some(cached_verses) = state.cache.get::<Vec<serde_json::Value>>(&cache_key).await {
        tracing::debug!("Returning cached verses");
        return Json(cached_verses).into_response();
    }
    
    tracing::debug!("Cache miss - building verses list");
    
    // Try to get verses from real Polymarket data first
    let verses: Vec<serde_json::Value> = match fetch_polymarket_markets(&state, 100).await {
        Ok(polymarket_markets) => {
            // Generate verses dynamically from real market data
            generate_verses_from_markets(&polymarket_markets)
        }
        Err(e) => {
            tracing::warn!("Failed to fetch Polymarket data for verses: {}, using static catalog", e);
            // Fallback to static catalog
            crate::verse_catalog::VERSE_CATALOG
                .iter()
                .map(|(_, verse)| json!({
                    "id": verse.id,
                    "name": verse.name,
                    "description": verse.description,
                    "level": verse.level,
                    "multiplier": verse.multiplier,
                    "category": verse.category,
                    "risk_tier": verse.risk_tier,
                    "parent_id": verse.parent_id,
                    "market_count": verse.market_count,
                }))
                .collect()
        }
    };
    
    // Cache for 15 minutes (verses don't change often)
    if let Err(e) = state.cache.set(&cache_key, &verses, Some(900)).await {
        tracing::warn!("Failed to cache verses: {}", e);
    }
    
    Json(verses).into_response()
}

/// Get a specific verse
pub async fn get_verse(
    Path(id): Path<String>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // Look up verse in the comprehensive catalog
    if let Some(verse) = crate::verse_catalog::VERSE_CATALOG.get(&id) {
        Json(json!({
            "id": verse.id,
            "name": verse.name,
            "description": verse.description,
            "level": verse.level,
            "multiplier": verse.multiplier,
            "category": verse.category,
            "risk_tier": verse.risk_tier,
            "parent_id": verse.parent_id,
            "market_count": verse.market_count,
        })).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({
            "error": {
                "code": "VERSE_NOT_FOUND",
                "message": "Verse not found",
            }
        }))).into_response()
    }
}

/// Get quantum positions
pub async fn get_quantum_positions(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.quantum_engine.get_wallet_positions(&wallet).await {
        Ok(quantum_positions) => {
            // Calculate portfolio metrics
            let metrics = match state.quantum_engine.calculate_quantum_metrics(&wallet).await {
                Ok(m) => Some(m),
                Err(_) => None,
            };
            
            Json(json!({
                "wallet": wallet,
                "quantum_positions": quantum_positions,
                "portfolio_metrics": metrics,
                "total": quantum_positions.len(),
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "QUANTUM_POSITIONS_ERROR",
                    "message": format!("Failed to get quantum positions: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Create a quantum position
pub async fn create_quantum_position(
    State(state): State<AppState>,
    Json(payload): Json<CreateQuantumPositionRequest>,
) -> impl IntoResponse {
    // Convert request states to quantum engine states
    let quantum_states: Vec<crate::quantum_engine::QuantumState> = payload.states.into_iter().map(|s| {
        crate::quantum_engine::QuantumState {
            market_id: s.market_id,
            outcome: s.outcome,
            amount: s.amount,
            leverage: s.leverage,
            amplitude: s.probability.sqrt(),
            phase: 0.0, // Default phase
            probability: s.probability,
            entangled_with: Vec::new(),
        }
    }).collect();
    
    // Create quantum position using the quantum engine
    match state.quantum_engine.create_quantum_position(
        "test-wallet".to_string(), // TODO: Extract from request or auth
        quantum_states,
        payload.entanglement_group,
    ).await {
        Ok(position_id) => {
            Json(json!({
                "quantum_position_id": position_id,
                "status": "created",
                "message": "Quantum position created successfully"
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "QUANTUM_POSITION_ERROR",
                    "message": format!("Failed to create quantum position: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Get quantum states for a market
pub async fn get_quantum_states(
    Path(market_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let market_id_u128 = match market_id.parse::<u128>() {
        Ok(id) => id,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_MARKET_ID",
                    "message": "Market ID must be a valid number",
                }
            }))).into_response();
        }
    };
    
    match state.quantum_engine.get_market_quantum_states(market_id_u128).await {
        Ok(quantum_states) => {
            Json(json!({
                "market_id": market_id,
                "quantum_states": quantum_states,
                "total_states": quantum_states.len(),
                "coherence": 0.95,
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "QUANTUM_STATES_ERROR",
                    "message": format!("Failed to get quantum states: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Stake MMT tokens
pub async fn stake_mmt(
    State(state): State<AppState>,
    Json(payload): Json<StakeMMTRequest>,
) -> impl IntoResponse {
    // Validate amount
    if payload.amount == 0 {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": {
                "code": "INVALID_AMOUNT",
                "message": "Stake amount must be greater than 0",
            }
        }))).into_response();
    }
    
    // Mock staking implementation
    let stake_id = Uuid::new_v4().to_string();
    let apy = match payload.duration {
        30 => 12.0,
        90 => 18.0,
        180 => 25.0,
        365 => 35.0,
        _ => 10.0,
    };
    
    Json(json!({
        "stake_id": stake_id,
        "amount": payload.amount,
        "duration": payload.duration,
        "apy": apy,
        "rewards_start": chrono::Utc::now().to_rfc3339(),
        "unlock_date": (chrono::Utc::now() + chrono::Duration::days(payload.duration)).to_rfc3339(),
        "message": "MMT tokens staked successfully",
    })).into_response()
}

/// Get liquidity pools
pub async fn get_liquidity_pools(State(_state): State<AppState>) -> impl IntoResponse {
    // Mock liquidity pools
    let pools = vec![
        json!({
            "pool_id": "pool_1",
            "name": "USDC/MMT",
            "tvl": 5000000,
            "apy": 25.5,
            "volume_24h": 1200000,
            "fee_tier": 0.3,
        }),
        json!({
            "pool_id": "pool_2",
            "name": "SOL/MMT",
            "tvl": 3000000,
            "apy": 32.0,
            "volume_24h": 800000,
            "fee_tier": 0.3,
        }),
        json!({
            "pool_id": "pool_3",
            "name": "ETH/MMT",
            "tvl": 2000000,
            "apy": 28.5,
            "volume_24h": 600000,
            "fee_tier": 0.5,
        }),
    ];
    
    Json(json!({
        "pools": pools,
        "total_tvl": 10000000,
        "total_volume_24h": 2600000,
    })).into_response()
}

/// Test verse matching endpoint
pub async fn test_verse_match(
    State(_state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let title = payload["title"].as_str().unwrap_or("");
    let category = payload["category"].as_str().unwrap_or("");
    let keywords: Vec<String> = payload["keywords"]
        .as_array()
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect())
        .unwrap_or_default();
    
    // Find matching verses
    let matching_verses = crate::verse_catalog::find_verses_for_market(title, category, &keywords);
    
    Json(json!({
        "input": {
            "title": title,
            "category": category,
            "keywords": keywords,
        },
        "matching_verses": matching_verses,
        "count": matching_verses.len(),
    })).into_response()
}

/// Handle margin call
pub async fn handle_margin_call(
    State(state): State<AppState>,
    Path(position_id): Path<String>,
) -> impl IntoResponse {
    // Parse position ID
    let position_pubkey = match Pubkey::from_str(&position_id) {
        Ok(pubkey) => pubkey,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": {
                    "code": "INVALID_POSITION_ID",
                    "message": "Invalid position ID",
                }
            }))).into_response()
        }
    };
    
    // Get position details (mock for now)
    let position = json!({
        "id": position_id,
        "market_id": 1,
        "current_margin": 100000, // $0.10
        "required_margin": 500000, // $0.50
        "liquidation_price": 0.45,
        "current_price": 0.48,
    });
    
    // Calculate margin health
    let current_margin_val = position["current_margin"].as_u64().unwrap_or(0);
    let required_margin = position["required_margin"].as_u64().unwrap_or(1);
    let margin_ratio = current_margin_val as f64 / required_margin as f64;
    
    // Determine action needed
    let (status, action) = if margin_ratio < 0.25 {
        ("critical", "immediate_liquidation")
    } else if margin_ratio < 0.5 {
        ("warning", "add_margin_required")
    } else if margin_ratio < 0.75 {
        ("caution", "monitor_closely")
    } else {
        ("healthy", "no_action_required")
    };
    
    Json(json!({
        "position": position,
        "margin_health": {
            "ratio": margin_ratio,
            "status": status,
            "action": action,
        },
        "options": {
            "add_margin": {
                "minimum": required_margin - current_margin_val,
                "recommended": (required_margin as f64 * 1.5) as u64 - current_margin_val,
            },
            "reduce_position": {
                "recommended_reduction": if margin_ratio < 0.5 { 0.5 } else { 0.25 },
            },
        },
        "deadline": if margin_ratio < 0.5 {
            Some((chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339())
        } else {
            None
        },
    })).into_response()
}

/// Get position margin requirements
pub async fn get_margin_requirements(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let market_id = params.get("market_id")
        .and_then(|s| s.parse::<u128>().ok())
        .unwrap_or(0);
    
    let amount = params.get("amount")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1_000_000); // Default 1 USDC
    
    let leverage = params.get("leverage")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    
    let outcome = params.get("outcome")
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(0);
    
    // Calculate margin requirements
    let base_margin = amount / leverage as u64;
    let initial_margin = (base_margin as f64 * 1.1) as u64; // 10% buffer
    let maintenance_margin = (base_margin as f64 * 0.5) as u64; // 50% of initial
    let liquidation_threshold = (base_margin as f64 * 0.25) as u64; // 25% of initial
    
    // Get current market price (mock)
    let current_price = 0.52;
    let entry_price = current_price;
    
    // Calculate liquidation price
    let liquidation_price = if outcome == 0 {
        // Long position
        entry_price * (1.0 - (liquidation_threshold as f64 / amount as f64))
    } else {
        // Short position
        entry_price * (1.0 + (liquidation_threshold as f64 / amount as f64))
    };
    
    Json(json!({
        "market_id": market_id,
        "amount": amount,
        "leverage": leverage,
        "outcome": outcome,
        "requirements": {
            "initial_margin": initial_margin,
            "maintenance_margin": maintenance_margin,
            "liquidation_threshold": liquidation_threshold,
        },
        "prices": {
            "current": current_price,
            "entry": entry_price,
            "liquidation": liquidation_price,
        },
        "risk_metrics": {
            "max_loss": amount,
            "margin_ratio": 1.0, // Default safe margin ratio
            "distance_to_liquidation": ((current_price - liquidation_price).abs() / current_price * 100.0),
        },
    })).into_response()
}


/// Place limit order
pub async fn place_limit_order(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Extract parameters
    let market_id = payload["market_id"].as_u64().unwrap_or(0) as u128;
    let wallet = payload["wallet"].as_str().unwrap_or("").to_string();
    let amount = payload["amount"].as_u64().unwrap_or(0);
    let outcome = payload["outcome"].as_u64().unwrap_or(0) as u8;
    let leverage = payload["leverage"].as_u64().unwrap_or(1) as u32;
    let price = payload["price"].as_f64().unwrap_or(0.0);
    
    // Validate parameters
    if wallet.is_empty() || amount == 0 || price <= 0.0 {
        return Json(serde_json::json!({
            "error": {
                "code": "INVALID_REQUEST",
                "message": "Missing or invalid parameters"
            }
        })).into_response();
    }
    
    // Create order
    let order = crate::order_types::Order {
        id: uuid::Uuid::new_v4().to_string(),
        market_id,
        wallet,
        order_type: crate::order_types::OrderType::Limit { price },
        side: if payload["side"].as_str() == Some("sell") {
            crate::order_types::OrderSide::Sell
        } else {
            crate::order_types::OrderSide::Buy
        },
        amount,
        outcome,
        leverage,
        status: crate::order_types::OrderStatus::Pending,
        time_in_force: crate::order_types::TimeInForce::GTC,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        filled_amount: 0,
        average_fill_price: None,
        fees: 0,
        verse_id: payload["verse_id"].as_str().map(String::from),
        metadata: std::collections::HashMap::new(),
    };
    
    // Validate order
    if let Err(e) = crate::order_types::validate_order(&order) {
        return Json(serde_json::json!({
            "error": {
                "code": "INVALID_ORDER",
                "message": e
            }
        })).into_response();
    }
    
    // Place order
    let engine = &state.order_engine;
    match engine.place_order(order) {
        Ok(placed_order) => Json(serde_json::json!({
            "order": placed_order,
            "message": "Limit order placed successfully"
        })).into_response(),
        Err(e) => Json(serde_json::json!({
            "error": {
                "code": "ORDER_FAILED",
                "message": e
            }
        })).into_response()
    }
}

/// Place stop order
pub async fn place_stop_order(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Extract parameters
    let market_id = payload["market_id"].as_u64().unwrap_or(0) as u128;
    let wallet = payload["wallet"].as_str().unwrap_or("").to_string();
    let amount = payload["amount"].as_u64().unwrap_or(0);
    let outcome = payload["outcome"].as_u64().unwrap_or(0) as u8;
    let leverage = payload["leverage"].as_u64().unwrap_or(1) as u32;
    let trigger_price = payload["trigger_price"].as_f64().unwrap_or(0.0);
    let order_type_str = payload["order_type"].as_str().unwrap_or("stop_loss");
    
    // Validate parameters
    if wallet.is_empty() || amount == 0 || trigger_price <= 0.0 {
        return Json(serde_json::json!({
            "error": {
                "code": "INVALID_REQUEST",
                "message": "Missing or invalid parameters"
            }
        })).into_response();
    }
    
    // Determine order type
    let order_type = if order_type_str == "take_profit" {
        crate::order_types::OrderType::TakeProfit { trigger_price }
    } else {
        crate::order_types::OrderType::StopLoss { trigger_price }
    };
    
    // Create order
    let order = crate::order_types::Order {
        id: uuid::Uuid::new_v4().to_string(),
        market_id,
        wallet,
        order_type,
        side: if payload["side"].as_str() == Some("sell") {
            crate::order_types::OrderSide::Sell
        } else {
            crate::order_types::OrderSide::Buy
        },
        amount,
        outcome,
        leverage,
        status: crate::order_types::OrderStatus::Pending,
        time_in_force: crate::order_types::TimeInForce::GTC,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        filled_amount: 0,
        average_fill_price: None,
        fees: 0,
        verse_id: payload["verse_id"].as_str().map(String::from),
        metadata: std::collections::HashMap::new(),
    };
    
    // Validate order
    if let Err(e) = crate::order_types::validate_order(&order) {
        return Json(serde_json::json!({
            "error": {
                "code": "INVALID_ORDER",
                "message": e
            }
        })).into_response();
    }
    
    // Place order
    let engine = &state.order_engine;
    match engine.place_order(order) {
        Ok(placed_order) => Json(serde_json::json!({
            "order": placed_order,
            "message": "Stop order placed successfully"
        })).into_response(),
        Err(e) => Json(serde_json::json!({
            "error": {
                "code": "ORDER_FAILED",
                "message": e
            }
        })).into_response()
    }
}

/// Cancel order
pub async fn cancel_order(
    Path(order_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let engine = &state.order_engine;
    
    match engine.cancel_order(&order_id) {
        Ok(cancelled_order) => {
            Json(serde_json::json!({
                "order": cancelled_order,
                "message": "Order cancelled successfully"
            })).into_response()
        }
        Err(e) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": {
                    "code": "ORDER_NOT_FOUND",
                    "message": e
                }
            }))).into_response()
        }
    }
}

/// Get orders for wallet
pub async fn get_orders(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let engine = &state.order_engine;
    let orders = engine.get_orders_by_wallet(&wallet);
    
    Json(serde_json::json!({
        "orders": orders,
        "count": orders.len()
    })).into_response()
}

/// Generate challenge for wallet verification
pub async fn generate_wallet_challenge(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Validate wallet format
    if !wallet.starts_with("demo-") && !wallet.starts_with("advanced-") && !wallet.starts_with("pro-") {
        // Try to parse as Solana public key
        if let Err(_) = Pubkey::from_str(&wallet) {
            return responses::bad_request("Invalid wallet format").into_response();
        }
    }
    
    match state.wallet_verification.generate_challenge(&wallet).await {
        Ok(challenge_response) => {
            Json(json!({
                "success": true,
                "challenge": challenge_response.challenge_compat,
                "nonce": challenge_response.nonce,
                "message": challenge_response.message,
                "expires_at": challenge_response.expires_at,
            })).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "CHALLENGE_GENERATION_ERROR",
                    "message": format!("Failed to generate challenge: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Verify wallet signature
pub async fn verify_wallet_signature(
    State(state): State<AppState>,
    Json(payload): Json<crate::wallet_verification::VerificationRequest>,
) -> impl IntoResponse {
    match state.wallet_verification.verify_signature(payload).await {
        Ok(response) => {
            if response.verified {
                Json(json!({
                    "success": true,
                    "verified": true,
                    "token": response.token,
                    "expires_at": response.expires_at,
                    "wallet": response.wallet,
                })).into_response()
            } else {
                (StatusCode::UNAUTHORIZED, Json(json!({
                    "success": false,
                    "verified": false,
                    "error": {
                        "code": "SIGNATURE_VERIFICATION_FAILED",
                        "message": "Invalid signature or expired challenge",
                    }
                }))).into_response()
            }
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": {
                    "code": "VERIFICATION_ERROR",
                    "message": format!("Verification failed: {}", e),
                }
            }))).into_response()
        }
    }
}

/// Check wallet verification status
pub async fn check_wallet_verification(
    Path(wallet): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let is_verified = state.wallet_verification.is_wallet_verified(&wallet).await;
    
    Json(json!({
        "wallet": wallet,
        "verified": is_verified,
    })).into_response()
}

/// Helper function to fetch markets from Polymarket and convert to internal format
async fn fetch_polymarket_markets(state: &AppState, limit: usize) -> Result<Vec<crate::types::Market>, anyhow::Error> {
    // Use the public Polymarket client
    let polymarket_markets = state.polymarket_public_client.get_current_markets(limit).await?;
    
    // Check if we're getting outdated markets (e.g., 2020 markets)
    let has_current_markets = polymarket_markets.iter().any(|m| {
        m.question.contains("2024") || m.question.contains("2025") || 
        m.question.contains("2026") || m.question.contains("2027")
    });
    
    // If all markets are old, use mock current markets instead
    if !has_current_markets && !polymarket_markets.is_empty() {
        tracing::info!("Polymarket API returned only historical markets, using mock current markets");
        return Ok(crate::mock_current_markets::get_mock_current_markets());
    }
    
    // Convert Polymarket markets to our internal format
    let mut internal_markets = Vec::new();
    for (index, pm_market) in polymarket_markets.into_iter().enumerate() {
        let market_json = pm_market.to_internal_format();
        
        // Convert JSON to our Market struct
        let market = crate::types::Market {
            id: index as u128 + 1000, // Use offset to avoid conflicts with seeded markets
            title: market_json["title"].as_str().unwrap_or("Unknown Market").to_string(),
            description: market_json["description"].as_str().unwrap_or("").to_string(),
            creator: solana_sdk::pubkey::Pubkey::default(), // Default pubkey for Polymarket markets
            outcomes: market_json["outcomes"].as_array().unwrap_or(&vec![]).iter().enumerate().map(|(i, outcome)| {
                crate::types::MarketOutcome {
                    id: i as u8,
                    name: outcome["name"].as_str().unwrap_or("Unknown").to_string(),
                    title: outcome["name"].as_str().unwrap_or("Unknown").to_string(),
                    description: format!("{} outcome", outcome["name"].as_str().unwrap_or("Unknown")),
                    total_stake: outcome["total_stake"].as_i64().unwrap_or(0) as u64,
                }
            }).collect(),
            amm_type: crate::types::AmmType::Hybrid, // Default to Hybrid for Polymarket markets
            total_volume: (market_json["total_volume"].as_f64().unwrap_or(0.0) * 1000000.0) as u64, // Convert to microunits
            total_liquidity: (market_json["total_liquidity"].as_f64().unwrap_or(0.0) * 1000000.0) as u64, // Convert to microunits
            resolution_time: chrono::Utc::now().timestamp() + 86400, // Default to 1 day from now
            resolved: market_json["closed"].as_bool().unwrap_or(false),
            winning_outcome: None,
            created_at: chrono::Utc::now().timestamp(),
            verse_id: Some(market_json["verse_id"].as_i64().unwrap_or(50) as u128),
            current_price: 0.5, // Default price
        };
        
        internal_markets.push(market);
    }
    
    // If we have no markets at all, use mock markets
    if internal_markets.is_empty() {
        tracing::info!("No markets from Polymarket, using mock current markets");
        return Ok(crate::mock_current_markets::get_mock_current_markets());
    }
    
    Ok(internal_markets)
}

/// Generate verses dynamically from real market data
fn generate_verses_from_markets(markets: &[crate::types::Market]) -> Vec<serde_json::Value> {
    use std::collections::HashMap;
    
    tracing::info!("Generating verses from {} markets", markets.len());
    
    // Count markets by verse_id
    let mut verse_counts: HashMap<u128, usize> = HashMap::new();
    for market in markets {
        if let Some(verse_id) = market.verse_id {
            *verse_counts.entry(verse_id).or_insert(0) += 1;
            tracing::debug!("Market '{}' has verse_id {}", market.title, verse_id);
        }
    }
    
    tracing::info!("Found {} unique verse IDs with markets", verse_counts.len());
    
    // Create verses based on real market data
    let mut verses = Vec::new();
    
    // Define verse categories based on Polymarket market patterns
    let verse_definitions = vec![
        (1u128, "Politics", "Political events and elections", 2.5, "low"),
        (2u128, "Crypto", "Cryptocurrency and DeFi markets", 3.2, "high"),
        (3u128, "Sports", "Sports betting and competitions", 1.8, "medium"),
        (4u128, "Entertainment", "Entertainment and pop culture", 2.0, "low"),
        (5u128, "Business", "Business and stock market predictions", 2.1, "medium"),
        (6u128, "Space", "Space exploration and technology", 2.8, "high"),
        (9u128, "Technology", "Technology and innovation predictions", 2.4, "medium"),
        (10u128, "Sports", "Sports betting and competitions", 1.8, "medium"),
        (11u128, "Environmental", "Climate and environmental events", 2.2, "medium"),
        (20u128, "Crypto", "Cryptocurrency and DeFi markets", 3.2, "high"),
        (30u128, "Finance", "Traditional finance and markets", 2.1, "medium"),
        (40u128, "Entertainment", "Entertainment and pop culture", 2.0, "low"),
        (50u128, "General", "General prediction markets", 1.5, "low"),
    ];
    
    for (id, name, description, multiplier, risk_tier) in verse_definitions {
        let market_count = verse_counts.get(&id).copied().unwrap_or(0);
        
        // Only include verses that have markets
        if market_count > 0 {
            verses.push(json!({
                "id": id,
                "name": name,
                "description": description,
                "level": 1,
                "multiplier": multiplier,
                "category": name.to_lowercase(),
                "risk_tier": risk_tier,
                "parent_id": null,
                "market_count": market_count,
                "source": "polymarket_live"
            }));
        }
    }
    
    // If no verses found, add a default general verse
    if verses.is_empty() {
        verses.push(json!({
            "id": 50u128,
            "name": "General",
            "description": "General prediction markets",
            "level": 1,
            "multiplier": 1.5,
            "category": "general",
            "risk_tier": "low",
            "parent_id": null,
            "market_count": markets.len(),
            "source": "polymarket_live"
        }));
    }
    
    tracing::info!("Generated {} verses", verses.len());
    verses
}
