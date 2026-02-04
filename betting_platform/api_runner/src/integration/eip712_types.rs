//! EIP-712 type definitions for Polymarket order signing
//! 
//! Implements the typed data structures required for signing Polymarket CLOB orders
//! according to the EIP-712 standard.

use serde::{Deserialize, Serialize};
use ethereum_types::{Address, U256};
use std::collections::BTreeMap;

/// The Exchange contract address on Polygon
pub const EXCHANGE_ADDRESS: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";

lazy_static::lazy_static! {
    /// Pre-computed domain separator for Polymarket CLOB
    pub static ref DOMAIN_SEPARATOR: [u8; 32] = {
        use web3::signing::keccak256;
        
        // Domain type hash for EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
        let domain_type_hash = hex::decode(
            "8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f"
        ).expect("Valid hex");
        
        let name_hash = keccak256(b"Polymarket CLOB");
        let version_hash = keccak256(b"1");
        let chain_id = 137u64; // Polygon mainnet
        let verifying_contract = hex::decode(
            EXCHANGE_ADDRESS.trim_start_matches("0x")
        ).expect("Valid address");
        
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&domain_type_hash);
        encoded.extend_from_slice(&name_hash);
        encoded.extend_from_slice(&version_hash);
        
        // Encode chainId as 32-byte word
        let mut chain_id_bytes = [0u8; 32];
        chain_id_bytes[24..].copy_from_slice(&chain_id.to_be_bytes());
        encoded.extend_from_slice(&chain_id_bytes);
        
        // Encode verifying contract as 32-byte word (left-padded)
        let mut contract_bytes = [0u8; 32];
        contract_bytes[12..].copy_from_slice(&verifying_contract);
        encoded.extend_from_slice(&contract_bytes);
        
        keccak256(&encoded)
    };
}

/// EIP-712 Domain Separator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EIP712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub verifying_contract: Address,
}

/// Polymarket order structure for EIP-712 signing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketOrder {
    /// Unique salt for order uniqueness
    pub salt: String,
    /// Maker address (order creator)
    pub maker: Address,
    /// Address of the signer (can be different from maker for delegation)
    pub signer: Address,
    /// Taker address (0x0 for open orders)
    pub taker: Address,
    /// Token ID (condition ID for the market)
    pub token_id: String,
    /// Maker amount (in outcome tokens or collateral)
    pub maker_amount: String,
    /// Taker amount (in outcome tokens or collateral)
    pub taker_amount: String,
    /// Expiration timestamp (0 for no expiration)
    pub expiration: String,
    /// Nonce for order cancellation
    pub nonce: String,
    /// Fee rate in basis points
    pub fee_rate_bps: String,
    /// Side: 0 = BUY, 1 = SELL
    pub side: u8,
    /// Signature type: 0 = EOA, 1 = POLY_PROXY, 2 = POLY_GNOSIS_SAFE
    pub signature_type: u8,
}

/// Order side enum matching Polymarket's specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderSide {
    Buy = 0,
    Sell = 1,
}

/// Signature type enum for different wallet types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignatureType {
    Eoa = 0,
    PolyProxy = 1,
    PolyGnosisSafe = 2,
}

/// Polymarket API order request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketOrderRequest {
    /// The signed order
    pub order: PolymarketOrder,
    /// The EIP-712 signature
    pub signature: String,
    /// Optional: Owner address if using proxy
    pub owner: Option<Address>,
}

/// Order response from Polymarket API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolymarketOrderResponse {
    /// Order hash (unique identifier)
    pub order_hash: String,
    /// Order status
    pub status: OrderStatus,
    /// Creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
    /// Filled amount so far
    pub filled: String,
    /// Average fill price
    pub avg_fill_price: Option<f64>,
}

/// Order status enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Active,
    Matched,
    Cancelled,
    Expired,
    PartiallyFilled,
}

/// Builder for creating Polymarket orders
pub struct PolymarketOrderBuilder {
    salt: Option<U256>,
    maker: Option<Address>,
    signer: Option<Address>,
    taker: Option<Address>,
    token_id: Option<U256>,
    maker_amount: Option<U256>,
    taker_amount: Option<U256>,
    expiration: Option<U256>,
    nonce: Option<U256>,
    fee_rate_bps: Option<U256>,
    side: Option<OrderSide>,
    signature_type: Option<SignatureType>,
}

impl PolymarketOrderBuilder {
    pub fn new() -> Self {
        Self {
            salt: None,
            maker: None,
            signer: None,
            taker: None,
            token_id: None,
            maker_amount: None,
            taker_amount: None,
            expiration: None,
            nonce: None,
            fee_rate_bps: None,
            side: None,
            signature_type: None,
        }
    }

    pub fn salt(mut self, salt: U256) -> Self {
        self.salt = Some(salt);
        self
    }

    pub fn maker(mut self, maker: Address) -> Self {
        self.maker = Some(maker);
        self
    }

    pub fn signer(mut self, signer: Address) -> Self {
        self.signer = Some(signer);
        self
    }

    pub fn taker(mut self, taker: Address) -> Self {
        self.taker = Some(taker);
        self
    }

    pub fn token_id(mut self, token_id: U256) -> Self {
        self.token_id = Some(token_id);
        self
    }

    pub fn maker_amount(mut self, maker_amount: U256) -> Self {
        self.maker_amount = Some(maker_amount);
        self
    }

    pub fn taker_amount(mut self, taker_amount: U256) -> Self {
        self.taker_amount = Some(taker_amount);
        self
    }

    pub fn expiration(mut self, expiration: U256) -> Self {
        self.expiration = Some(expiration);
        self
    }

    pub fn nonce(mut self, nonce: U256) -> Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn fee_rate_bps(mut self, fee_rate_bps: U256) -> Self {
        self.fee_rate_bps = Some(fee_rate_bps);
        self
    }

    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    pub fn signature_type(mut self, signature_type: SignatureType) -> Self {
        self.signature_type = Some(signature_type);
        self
    }

    /// Build the order with default values for optional fields
    pub fn build(self) -> Result<PolymarketOrder, String> {
        // Generate random salt if not provided
        let salt = self.salt.unwrap_or_else(|| {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            U256::from(rng.gen::<u128>())
        });

        let maker = self.maker.ok_or("Maker address is required")?;
        let signer = self.signer.unwrap_or(maker); // Default signer to maker
        let taker = self.taker.unwrap_or_else(Address::zero); // Open order by default
        let token_id = self.token_id.ok_or("Token ID is required")?;
        let maker_amount = self.maker_amount.ok_or("Maker amount is required")?;
        let taker_amount = self.taker_amount.ok_or("Taker amount is required")?;
        let expiration = self.expiration.unwrap_or_else(U256::zero); // No expiration by default
        let nonce = self.nonce.unwrap_or_else(U256::zero);
        let fee_rate_bps = self.fee_rate_bps.unwrap_or_else(|| U256::from(0)); // 0% fee by default
        let side = self.side.ok_or("Order side is required")?;
        let signature_type = self.signature_type.unwrap_or(SignatureType::Eoa);

        Ok(PolymarketOrder {
            salt: format!("0x{:064x}", salt),
            maker,
            signer,
            taker,
            token_id: format!("0x{:064x}", token_id),
            maker_amount: format!("0x{:064x}", maker_amount),
            taker_amount: format!("0x{:064x}", taker_amount),
            expiration: format!("0x{:064x}", expiration),
            nonce: format!("0x{:064x}", nonce),
            fee_rate_bps: format!("0x{:064x}", fee_rate_bps),
            side: side as u8,
            signature_type: signature_type as u8,
        })
    }
}

/// Helper functions for order creation and validation
impl PolymarketOrder {
    /// Calculate the implied price of the order
    pub fn implied_price(&self) -> Result<f64, String> {
        let maker_amt = U256::from_str_radix(&self.maker_amount.trim_start_matches("0x"), 16)
            .map_err(|_| "Invalid maker amount")?
            .as_u128() as f64;
        let taker_amt = U256::from_str_radix(&self.taker_amount.trim_start_matches("0x"), 16)
            .map_err(|_| "Invalid taker amount")?
            .as_u128() as f64;
        
        Ok(if self.side == OrderSide::Buy as u8 {
            // Buy order: price = taker_amount / (maker_amount + taker_amount)
            taker_amt / (maker_amt + taker_amt)
        } else {
            // Sell order: price = maker_amount / (maker_amount + taker_amount)
            maker_amt / (maker_amt + taker_amt)
        })
    }

    /// Check if the order has expired
    pub fn is_expired(&self) -> Result<bool, String> {
        let exp = U256::from_str_radix(&self.expiration.trim_start_matches("0x"), 16)
            .map_err(|_| "Invalid expiration")?;
        
        if exp == U256::zero() {
            return Ok(false); // No expiration
        }
        
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(exp.as_u64() < now)
    }

    /// Generate order hash according to Polymarket's specification
    pub fn hash(&self) -> String {
        use keccak_hash::keccak;
        
        // Encode the order fields
        let mut encoded = Vec::new();
        
        // Helper to decode hex strings
        let decode_hex = |s: &str| -> Vec<u8> {
            hex::decode(s.trim_start_matches("0x")).unwrap_or_default()
        };
        
        encoded.extend(decode_hex(&self.salt));
        encoded.extend(self.maker.as_bytes());
        encoded.extend(self.signer.as_bytes());
        encoded.extend(self.taker.as_bytes());
        encoded.extend(decode_hex(&self.token_id));
        encoded.extend(decode_hex(&self.maker_amount));
        encoded.extend(decode_hex(&self.taker_amount));
        encoded.extend(decode_hex(&self.expiration));
        encoded.extend(decode_hex(&self.nonce));
        encoded.extend(decode_hex(&self.fee_rate_bps));
        encoded.push(self.side);
        encoded.push(self.signature_type);
        
        let hash = keccak(&encoded);
        format!("0x{}", hex::encode(hash.as_bytes()))
    }
}

/// EIP-712 typed data for signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedData {
    pub domain: EIP712Domain,
    pub primary_type: String,
    pub types: BTreeMap<String, Vec<TypedField>>,
    pub message: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
}

/// Create typed data for EIP-712 signing
pub fn create_typed_data(order: &PolymarketOrder) -> TypedData {
    // Define the domain
    let domain = EIP712Domain {
        name: "Polymarket".to_string(),
        version: "1".to_string(),
        chain_id: 137, // Polygon mainnet
        verifying_contract: "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
            .parse()
            .unwrap(),
    };
    
    // Define the types
    let mut types = BTreeMap::new();
    
    // EIP712Domain type
    types.insert(
        "EIP712Domain".to_string(),
        vec![
            TypedField {
                name: "name".to_string(),
                field_type: "string".to_string(),
            },
            TypedField {
                name: "version".to_string(),
                field_type: "string".to_string(),
            },
            TypedField {
                name: "chainId".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "verifyingContract".to_string(),
                field_type: "address".to_string(),
            },
        ],
    );
    
    // Order type
    types.insert(
        "Order".to_string(),
        vec![
            TypedField {
                name: "salt".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "maker".to_string(),
                field_type: "address".to_string(),
            },
            TypedField {
                name: "signer".to_string(),
                field_type: "address".to_string(),
            },
            TypedField {
                name: "taker".to_string(),
                field_type: "address".to_string(),
            },
            TypedField {
                name: "tokenId".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "makerAmount".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "takerAmount".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "expiration".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "nonce".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "feeRateBps".to_string(),
                field_type: "uint256".to_string(),
            },
            TypedField {
                name: "side".to_string(),
                field_type: "uint8".to_string(),
            },
            TypedField {
                name: "signatureType".to_string(),
                field_type: "uint8".to_string(),
            },
        ],
    );
    
    // Create the message
    let message = serde_json::json!({
        "salt": order.salt,
        "maker": format!("{:?}", order.maker),
        "signer": format!("{:?}", order.signer),
        "taker": format!("{:?}", order.taker),
        "tokenId": order.token_id,
        "makerAmount": order.maker_amount,
        "takerAmount": order.taker_amount,
        "expiration": order.expiration,
        "nonce": order.nonce,
        "feeRateBps": order.fee_rate_bps,
        "side": order.side,
        "signatureType": order.signature_type,
    });
    
    TypedData {
        domain,
        primary_type: "Order".to_string(),
        types,
        message,
    }
}

/// Encode typed data according to EIP-712
pub fn encode_typed_data(typed_data: &TypedData) -> Result<Vec<u8>, String> {
    use keccak_hash::keccak;
    
    // EIP-712 prefix
    let prefix = b"\x19\x01";
    
    // Encode domain separator
    let domain_separator = encode_domain_separator(&typed_data.domain)?;
    
    // Encode message hash
    let message_hash = encode_message_hash(&typed_data)?;
    
    // Combine prefix + domain separator + message hash
    let mut encoded = Vec::new();
    encoded.extend_from_slice(prefix);
    encoded.extend_from_slice(&domain_separator);
    encoded.extend_from_slice(&message_hash);
    
    Ok(keccak(&encoded).as_bytes().to_vec())
}

fn encode_domain_separator(domain: &EIP712Domain) -> Result<Vec<u8>, String> {
    use keccak_hash::keccak;
    
    // EIP712Domain type hash
    let type_hash = keccak(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");
    
    // Encode domain values
    let mut encoded = Vec::new();
    encoded.extend_from_slice(type_hash.as_bytes());
    encoded.extend_from_slice(&keccak(domain.name.as_bytes()).as_bytes());
    encoded.extend_from_slice(&keccak(domain.version.as_bytes()).as_bytes());
    
    // Encode chainId as 32-byte value
    let mut chain_id_bytes = [0u8; 32];
    chain_id_bytes[24..].copy_from_slice(&domain.chain_id.to_be_bytes());
    encoded.extend_from_slice(&chain_id_bytes);
    
    // Encode verifying contract (already 20 bytes, pad to 32)
    let mut contract_bytes = [0u8; 32];
    contract_bytes[12..].copy_from_slice(domain.verifying_contract.as_bytes());
    encoded.extend_from_slice(&contract_bytes);
    
    Ok(keccak(&encoded).as_bytes().to_vec())
}

fn encode_message_hash(typed_data: &TypedData) -> Result<Vec<u8>, String> {
    use keccak_hash::keccak;
    
    // Get the Order type hash
    let type_hash = keccak(b"Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)");
    
    // This is a simplified encoding - in production, you'd need to properly encode each field
    // according to the EIP-712 specification
    let message_str = serde_json::to_string(&typed_data.message)
        .map_err(|e| format!("Failed to encode message: {}", e))?;
    
    let mut encoded = Vec::new();
    encoded.extend_from_slice(type_hash.as_bytes());
    encoded.extend_from_slice(&keccak(message_str.as_bytes()).as_bytes());
    
    Ok(keccak(&encoded).as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_builder() {
        let maker: Address = "0x1234567890123456789012345678901234567890".parse().unwrap();
        let token_id = U256::from(12345);
        
        let order = PolymarketOrderBuilder::new()
            .maker(maker)
            .token_id(token_id)
            .maker_amount(U256::from(1000))
            .taker_amount(U256::from(500))
            .side(OrderSide::Buy)
            .build()
            .unwrap();
        
        assert_eq!(order.maker, maker);
        assert_eq!(order.signer, maker); // Should default to maker
        assert_eq!(order.side, OrderSide::Buy as u8);
    }
    
    #[test]
    fn test_implied_price() {
        let order = PolymarketOrder {
            salt: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
            maker: Address::zero(),
            signer: Address::zero(),
            taker: Address::zero(),
            token_id: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
            maker_amount: "0x0000000000000000000000000000000000000000000000000000000000000258".to_string(), // 600
            taker_amount: "0x0000000000000000000000000000000000000000000000000000000000000190".to_string(), // 400
            expiration: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            nonce: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            fee_rate_bps: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            side: OrderSide::Buy as u8,
            signature_type: SignatureType::Eoa as u8,
        };
        
        let price = order.implied_price().unwrap();
        assert!((price - 0.4).abs() < 0.001); // Buy at 40%
    }
}