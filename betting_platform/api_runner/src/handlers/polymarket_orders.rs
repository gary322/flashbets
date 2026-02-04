use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::{
    AppState,
    typed_errors::{AppError, ErrorKind, ErrorContext},
    integration::{
        eip712_verifier::EIP712Verifier,
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitOrderRequest {
    pub order: OrderData,
    pub signature: String,
    #[serde(alias = "marketId")]
    pub market_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderData {
    pub salt: String,
    pub maker: String,
    pub signer: String,
    pub taker: String,
    pub token_id: String,
    pub maker_amount: String,
    pub taker_amount: String,
    pub expiration: String,
    pub nonce: String,
    pub fee_rate_bps: String,
    pub side: u8,
    pub signature_type: u8,
}

#[derive(Debug, Serialize)]
pub struct SubmitOrderResponse {
    pub order_id: String,
    pub order_hash: String,
    pub status: String,
    pub created_at: String,
    pub fills: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct OrderStatusResponse {
    pub order_id: String,
    pub status: String,
    pub filled_amount: String,
    pub remaining_amount: String,
    pub average_fill_price: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub fills: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
    Failed,
}

fn clob_status_to_string(status: &crate::integration::polymarket_clob::OrderStatus) -> &'static str {
    use crate::integration::polymarket_clob::OrderStatus as ClobStatus;

    match status {
        ClobStatus::Pending => "PENDING",
        ClobStatus::Open => "OPEN",
        ClobStatus::PartiallyFilled => "PARTIALLY_FILLED",
        ClobStatus::Filled => "FILLED",
        ClobStatus::Cancelled => "CANCELLED",
        ClobStatus::Expired => "EXPIRED",
        ClobStatus::Failed => "FAILED",
    }
}

/// Submit a signed order to Polymarket
pub async fn submit_order(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SubmitOrderRequest>,
) -> Result<Json<SubmitOrderResponse>, AppError> {
    info!(
        market_id = %request.market_id,
        side = request.order.side,
        "Received order submission request"
    );

    // Convert to PolymarketOrder for verification
    let order = request.order.to_polymarket_order()
        .map_err(|e| AppError::new(
            ErrorKind::ValidationError,
            format!("Invalid order data: {}", e),
            ErrorContext::new("polymarket_orders", "submit_order"),
        ))?;

    // Verify the signature
    match EIP712Verifier::verify_order_signature(&order, &request.signature) {
        Ok(true) => {
            info!("Order signature verified successfully");
        }
        Ok(false) => {
            warn!("Invalid order signature");
            return Err(AppError::new(
                ErrorKind::Unauthorized,
                "Invalid order signature",
                ErrorContext::new("polymarket_orders", "submit_order"),
            ));
        }
        Err(e) => {
            error!("Error verifying signature: {}", e);
            return Err(AppError::new(
                ErrorKind::ValidationError,
                format!("Signature verification failed: {}", e),
                ErrorContext::new("polymarket_orders", "submit_order"),
            ));
        }
    }

    // Validate order parameters
    if let Err(e) = EIP712Verifier::validate_order(&order) {
        return Err(AppError::new(
            ErrorKind::ValidationError,
            format!("Order validation failed: {}", e),
            ErrorContext::new("polymarket_orders", "submit_order"),
        ));
    }

    let clob_client = state.polymarket_clob_client
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Polymarket CLOB client not configured. Set POLYMARKET_API_KEY/POLYMARKET_API_SECRET/POLYMARKET_API_PASSPHRASE and (optionally) POLYMARKET_CLOB_BASE_URL for demo mocks.",
            ErrorContext::new("polymarket_orders", "submit_order"),
        ))?;

    let clob_order_request = crate::integration::polymarket_clob::OrderRequest {
        order: crate::integration::polymarket_auth::PolymarketOrderData {
            salt: order.salt.clone(),
            maker: order.maker,
            signer: order.signer,
            taker: order.taker,
            token_id: order.token_id.clone(),
            maker_amount: order.maker_amount.clone(),
            taker_amount: order.taker_amount.clone(),
            expiration: order.expiration.clone(),
            nonce: order.nonce.clone(),
            fee_rate_bps: order.fee_rate_bps.clone(),
            side: order.side,
            signature_type: order.signature_type,
        },
        signature: request.signature.clone(),
        owner: Some(request.order.maker.clone()),
    };

    let submitted = clob_client
        .submit_order(clob_order_request)
        .await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Polymarket order submission failed: {}", e),
            ErrorContext::new("polymarket_orders", "submit_order"),
        ))?;

    let submission_result = SubmitOrderResponse {
        order_id: submitted.order_id.clone(),
        order_hash: submitted.order_hash.clone(),
        status: clob_status_to_string(&submitted.status).to_string(),
        created_at: submitted.created_at.to_rfc3339(),
        fills: vec![],
    };
    
    // Store order in database for tracking
    if let Ok(pool) = state.database.get_pool() {
        if let Ok(client) = pool.get().await {
            let outcome: Option<i16> = None;
            let _ = client.execute(
                r#"
                INSERT INTO polymarket_orders 
                    (order_id, order_hash, market_id, user_address, side, outcome, 
                     amount, price, status, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                &[
                    &submission_result.order_id,
                    &submission_result.order_hash,
                    &request.market_id,
                    &order.maker.to_string(),
                    &(order.side as i16),
                    &outcome,
                    &order.maker_amount,
                    &order.taker_amount,
                    &submission_result.status,
                    &chrono::Utc::now(),
                ],
            ).await;
        }
    }
    
    info!(
        order_id = %submission_result.order_id,
        order_hash = %submission_result.order_hash,
        "Order submitted to Polymarket successfully"
    );

    Ok(Json(submission_result))
}

/// Get order status
pub async fn get_order_status(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(order_id): axum::extract::Path<String>,
) -> Result<Json<OrderStatusResponse>, AppError> {
    info!(order_id = %order_id, "Getting order status");

    let clob_client = state.polymarket_clob_client
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Polymarket CLOB client not configured",
            ErrorContext::new("polymarket_orders", "get_order_status"),
        ))?;

    let order = clob_client
        .get_order(&order_id)
        .await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to fetch order status from Polymarket: {}", e),
            ErrorContext::new("polymarket_orders", "get_order_status"),
        ))?;

    Ok(Json(OrderStatusResponse {
        order_id: order.order_id,
        status: clob_status_to_string(&order.status).to_string(),
        filled_amount: order.filled_amount,
        remaining_amount: order.remaining_amount,
        average_fill_price: order.average_fill_price.map(|p| p.to_string()),
        created_at: order.created_at.to_rfc3339(),
        updated_at: order.updated_at.to_rfc3339(),
        fills: vec![],
    }))
}

/// Cancel an order
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(order_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    info!(order_id = %order_id, "Cancelling order");

    let clob_client = state.polymarket_clob_client
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Polymarket CLOB client not configured",
            ErrorContext::new("polymarket_orders", "cancel_order"),
        ))?;

    clob_client
        .cancel_order(&order_id)
        .await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to cancel order on Polymarket: {}", e),
            ErrorContext::new("polymarket_orders", "cancel_order"),
        ))?;
    
    // Update order status in database
    if let Ok(pool) = state.database.get_pool() {
        if let Ok(client) = pool.get().await {
            let _ = client.execute(
                "UPDATE polymarket_orders SET status = 'CANCELLED', updated_at = $1 WHERE order_id = $2",
                &[&chrono::Utc::now(), &order_id],
            ).await;
        }
    }
    
    Ok(Json(serde_json::json!({
        "order_id": order_id,
        "status": "CANCELLED",
        "message": "Order cancelled on Polymarket successfully"
    })))
}

/// Get user's open orders
pub async fn get_open_orders(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<GetOrdersParams>,
) -> Result<Json<Vec<OrderStatusResponse>>, AppError> {
    info!(address = ?params.address, "Getting open orders");

    let clob_client = state.polymarket_clob_client
        .as_ref()
        .ok_or_else(|| AppError::new(
            ErrorKind::ServiceUnavailable,
            "Polymarket CLOB client not configured",
            ErrorContext::new("polymarket_orders", "get_open_orders"),
        ))?;

    let orders = clob_client
        .get_open_orders(crate::integration::polymarket_clob::OrdersQuery {
            address: params.address.clone(),
            market: params.market_id.clone(),
            status: Some("OPEN".to_string()),
            limit: params.limit,
            offset: params.offset,
        })
        .await
        .map_err(|e| AppError::new(
            ErrorKind::ExternalServiceError,
            format!("Failed to fetch open orders from Polymarket: {}", e),
            ErrorContext::new("polymarket_orders", "get_open_orders"),
        ))?;

    Ok(Json(orders.into_iter().map(|order| OrderStatusResponse {
        order_id: order.order_id,
        status: clob_status_to_string(&order.status).to_string(),
        filled_amount: order.filled_amount,
        remaining_amount: order.remaining_amount,
        average_fill_price: order.average_fill_price.map(|p| p.to_string()),
        created_at: order.created_at.to_rfc3339(),
        updated_at: order.updated_at.to_rfc3339(),
        fills: vec![],
    }).collect()))
}

#[derive(Debug, Deserialize)]
pub struct GetOrdersParams {
    pub address: String,
    pub market_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl OrderData {
    /// Convert to PolymarketOrder for verification
    pub fn to_polymarket_order(&self) -> Result<crate::integration::eip712_types::PolymarketOrder, String> {
        use ethereum_types::Address;
        use std::str::FromStr;

        Ok(crate::integration::eip712_types::PolymarketOrder {
            salt: self.salt.clone(),
            maker: Address::from_str(&self.maker)
                .map_err(|e| format!("Invalid maker address: {}", e))?,
            signer: Address::from_str(&self.signer)
                .map_err(|e| format!("Invalid signer address: {}", e))?,
            taker: Address::from_str(&self.taker)
                .map_err(|e| format!("Invalid taker address: {}", e))?,
            token_id: self.token_id.clone(),
            maker_amount: self.maker_amount.clone(),
            taker_amount: self.taker_amount.clone(),
            expiration: self.expiration.clone(),
            nonce: self.nonce.clone(),
            fee_rate_bps: self.fee_rate_bps.clone(),
            side: self.side,
            signature_type: self.signature_type,
        })
    }
}
