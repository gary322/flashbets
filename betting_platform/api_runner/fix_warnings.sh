#!/bin/bash
# Script to fix compiler warnings systematically

echo "Fixing unused imports in verse_generator.rs..."
sed -i '' 's/use std::collections::{HashMap, HashSet};/use std::collections::HashSet;/' src/verse_generator.rs

echo "Fixing unused imports in auth.rs..."
sed -i '' '/use axum::{$/,/};$/{
    s/, FromRequest//
    s/, State//
    s/, Request//
    s/, RequestExt//
}' src/auth.rs
sed -i '' '/use std::sync::Arc;/d' src/auth.rs

echo "Fixing unused imports in wallet_utils.rs..."
sed -i '' 's/use solana_sdk::hash::{hash, Hash};/use solana_sdk::hash::hash;/' src/wallet_utils.rs

echo "Fixing unused imports in wallet_verification.rs..."
sed -i '' 's/use solana_sdk::signer::{Signer, keypair::Keypair};/use solana_sdk::signer::Signer;/' src/wallet_verification.rs

echo "Fixing unused imports in seed_markets.rs..."
sed -i '' '/use tracing::info;/d' src/seed_markets.rs

echo "Fixing unused imports in websocket/enhanced.rs..."
sed -i '' 's/use tracing::{info, warn, error, debug};/use tracing::{info, warn, error};/' src/websocket/enhanced.rs
sed -i '' '/use crate::types::WsMessage;/d' src/websocket/enhanced.rs
sed -i '' '/use crate::rpc_client::BettingPlatformClient;/d' src/websocket/enhanced.rs

echo "Fixing unused imports in websocket/real_events.rs..."
sed -i '' '/use std::sync::Arc;/d' src/websocket/real_events.rs
sed -i '' 's/use crate::{integration, types::MarketOutcome};/use crate::types::MarketOutcome;/' src/websocket/real_events.rs
sed -i '' '/use solana_sdk::pubkey::Pubkey;/d' src/websocket/real_events.rs

echo "Fixing unused imports in validation.rs..."
sed -i '' 's/use serde::{Serialize, Deserialize};/use serde::Deserialize;/' src/validation.rs

echo "Fixing unused imports in cache.rs..."
sed -i '' 's/use redis::{Connection, AsyncCommands};/use redis::AsyncCommands;/' src/cache.rs
sed -i '' 's/use serde::{Serialize, Deserialize};/use serde::Serialize;/' src/cache.rs
sed -i '' 's/use std::time::Duration;//' src/cache.rs

echo "Fixing unused imports in solana_funding.rs..."
sed -i '' '/use std::str::FromStr;/d' src/solana_funding.rs

echo "Fixing unused imports in risk_engine_ext.rs..."
sed -i '' '/use std::collections::HashMap;/d' src/risk_engine_ext.rs
sed -i '' '/use tokio::sync::RwLock;/d' src/risk_engine_ext.rs
sed -i '' '/use std::sync::Arc;/d' src/risk_engine_ext.rs

echo "Fixing unused imports in quantum_engine_ext.rs..."
sed -i '' '/use crate::auth::{State};/d' src/quantum_engine_ext.rs
sed -i '' '/use std::sync::Arc;/d' src/quantum_engine_ext.rs

echo "Fixing unused imports in staking_handlers.rs..."
sed -i '' 's/use tracing::{info, warn, error};/use tracing::{info, warn};/' src/staking_handlers.rs

echo "Fixing unused imports in risk_handlers.rs..."
sed -i '' 's/use tracing::{info, warn, error};/use tracing::{info, warn};/' src/risk_handlers.rs

echo "Fixing unused imports in quantum_handlers.rs..."
sed -i '' 's/use anyhow::{anyhow, Result};/use anyhow::Result;/' src/quantum_handlers.rs

echo "Fixing unused imports in db/market_queries.rs..."
sed -i '' '/use std::collections::HashMap;/d' src/db/market_queries.rs

echo "Fixing unused imports in db/order_queries.rs..."
sed -i '' 's/use tracing::{info, warn, error, debug};/use tracing::{info, warn};/' src/db/order_queries.rs

echo "Fixing unused imports in db/position_queries.rs..."
sed -i '' 's/use tracing::{info, warn, error};/use tracing::{info};/' src/db/position_queries.rs

echo "Fixing unused imports in db/user_queries.rs..."
sed -i '' '/use solana_sdk::pubkey::Pubkey;/d' src/db/user_queries.rs

echo "Fixing unused imports in db/settlement_queries.rs..."
sed -i '' 's/use tokio::time::{interval, Duration};/use tokio::time::interval;/' src/db/settlement_queries.rs

echo "Fixing unused imports in response_types.rs..."
sed -i '' 's/use hyper::{Body, StatusCode, Response, HeaderMap};/use hyper::{Body, Response, HeaderMap};/' src/response_types.rs

echo "Fixing unused imports in auth_endpoints.rs..."
sed -i '' 's/use axum::http::{StatusCode, header};/use axum::http::header;/' src/auth_endpoints.rs

echo "Fixing unused imports in rbac_endpoints.rs..."
sed -i '' '/use hyper::StatusCode;/d' src/rbac_endpoints.rs

echo "Fixing unused imports in integration/price_feed.rs..."
sed -i '' 's/use std::sync::{Arc, Mutex};/use std::sync::Arc;/' src/integration/price_feed.rs

echo "Fixing unused imports in handlers/handlers.rs..."
sed -i '' 's/use axum::extract::{Path, Query, State};/use axum::extract::{Query, State};/' src/handlers/handlers.rs

echo "Done fixing unused imports!"