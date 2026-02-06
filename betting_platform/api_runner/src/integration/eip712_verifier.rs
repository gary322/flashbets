use ethereum_types::{Address, U256};
use web3::signing::{keccak256, recover};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::eip712_types::{PolymarketOrder, EIP712Domain, DOMAIN_SEPARATOR};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedOrder {
    pub order: PolymarketOrder,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSubmission {
    pub order: OrderData,
    pub signature: String,
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

impl OrderData {
    /// Convert to PolymarketOrder for verification
    pub fn to_polymarket_order(&self) -> Result<PolymarketOrder, String> {
        Ok(PolymarketOrder {
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

pub struct EIP712Verifier;

impl EIP712Verifier {
    /// Verify an EIP-712 signed order
    pub fn verify_order_signature(
        order: &PolymarketOrder,
        signature: &str,
    ) -> Result<bool, String> {
        // Remove 0x prefix if present
        let sig_bytes = hex::decode(signature.trim_start_matches("0x"))
            .map_err(|e| format!("Invalid signature hex: {}", e))?;

        if sig_bytes.len() != 65 {
            return Err(format!("Invalid signature length: {} bytes", sig_bytes.len()));
        }

        // Extract r, s, v from signature
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&sig_bytes[0..32]);
        s.copy_from_slice(&sig_bytes[32..64]);
        let v = sig_bytes[64];

        // Compute the message hash according to EIP-712
        let message_hash = Self::compute_eip712_hash(order)?;

        // Recover the signer address
        let recovery_id = if v >= 27 { v - 27 } else { v };
        let recovered_address = recover(&message_hash, &sig_bytes[0..64], recovery_id as i32)
            .map_err(|e| format!("Failed to recover address: {:?}", e))?;

        // Verify the recovered address matches the order's signer
        Ok(recovered_address == order.signer)
    }

    /// Compute EIP-712 hash for an order
    fn compute_eip712_hash(order: &PolymarketOrder) -> Result<[u8; 32], String> {
        // Encode the order struct according to EIP-712
        let encoded_order = Self::encode_order(order)?;
        
        // Hash the encoded order
        let order_hash = keccak256(&encoded_order);

        // Compute the EIP-712 message hash
        // "\x19\x01" || domainSeparator || hashStruct(message)
        let mut message = Vec::with_capacity(66);
        message.push(0x19);
        message.push(0x01);
        message.extend_from_slice(&*DOMAIN_SEPARATOR);
        message.extend_from_slice(&order_hash);

        Ok(keccak256(&message))
    }

    /// Encode order data according to EIP-712 encoding rules
    fn encode_order(order: &PolymarketOrder) -> Result<Vec<u8>, String> {
        // EIP-712 encoding: ORDER_TYPE_HASH || abi.encode(order fields)
        // keccak256("Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)")
        let order_type_hash = keccak256(b"Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)");

        let mut encoded = Vec::new();
        encoded.extend_from_slice(&order_type_hash);

        // Encode each field as 32-byte word
        // salt (uint256)
        let salt = U256::from_dec_str(&order.salt)
            .map_err(|e| format!("Invalid salt: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(salt));

        // maker (address)
        encoded.extend_from_slice(&Self::encode_address(order.maker));

        // signer (address)
        encoded.extend_from_slice(&Self::encode_address(order.signer));

        // taker (address)
        encoded.extend_from_slice(&Self::encode_address(order.taker));

        // tokenId (uint256)
        let token_id = U256::from_dec_str(&order.token_id)
            .map_err(|e| format!("Invalid token_id: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(token_id));

        // makerAmount (uint256)
        let maker_amount = U256::from_dec_str(&order.maker_amount)
            .map_err(|e| format!("Invalid maker_amount: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(maker_amount));

        // takerAmount (uint256)
        let taker_amount = U256::from_dec_str(&order.taker_amount)
            .map_err(|e| format!("Invalid taker_amount: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(taker_amount));

        // expiration (uint256)
        let expiration = U256::from_dec_str(&order.expiration)
            .map_err(|e| format!("Invalid expiration: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(expiration));

        // nonce (uint256)
        let nonce = U256::from_dec_str(&order.nonce)
            .map_err(|e| format!("Invalid nonce: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(nonce));

        // feeRateBps (uint256)
        let fee_rate = U256::from_dec_str(&order.fee_rate_bps)
            .map_err(|e| format!("Invalid fee_rate_bps: {}", e))?;
        encoded.extend_from_slice(&Self::encode_uint256(fee_rate));

        // side (uint8)
        encoded.extend_from_slice(&Self::encode_uint8(order.side));

        // signatureType (uint8)
        encoded.extend_from_slice(&Self::encode_uint8(order.signature_type));

        Ok(encoded)
    }

    /// Encode address as 32-byte word (left-padded with zeros)
    fn encode_address(address: Address) -> [u8; 32] {
        let mut encoded = [0u8; 32];
        encoded[12..].copy_from_slice(address.as_bytes());
        encoded
    }

    /// Encode uint256 as 32-byte word (big-endian)
    fn encode_uint256(value: U256) -> [u8; 32] {
        let mut encoded = [0u8; 32];
        value.to_big_endian(&mut encoded);
        encoded
    }

    /// Encode uint8 as 32-byte word (left-padded with zeros)
    fn encode_uint8(value: u8) -> [u8; 32] {
        let mut encoded = [0u8; 32];
        encoded[31] = value;
        encoded
    }

    /// Validate order parameters before submission
    pub fn validate_order(order: &PolymarketOrder) -> Result<(), String> {
        // Check expiration
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let expiration = u64::from_str(&order.expiration)
            .map_err(|_| "Invalid expiration timestamp")?;
        
        if expiration <= now {
            return Err("Order has expired".to_string());
        }

        // Check amounts are positive
        let maker_amount = U256::from_dec_str(&order.maker_amount)
            .map_err(|_| "Invalid maker amount")?;
        let taker_amount = U256::from_dec_str(&order.taker_amount)
            .map_err(|_| "Invalid taker amount")?;
        
        if maker_amount.is_zero() || taker_amount.is_zero() {
            return Err("Order amounts must be positive".to_string());
        }

        // Check fee is reasonable (max 10%)
        let fee_rate = u64::from_str(&order.fee_rate_bps)
            .map_err(|_| "Invalid fee rate")?;
        
        if fee_rate > 1000 {
            return Err("Fee rate too high (max 10%)".to_string());
        }

        // Check side is valid
        if order.side > 1 {
            return Err("Invalid order side".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_encoding() {
        let addr = Address::from_str("0x742d35Cc6634C0532925a3b844Bc9e7595f8fA49").unwrap();
        let encoded = EIP712Verifier::encode_address(addr);
        
        // Should be left-padded with zeros
        assert_eq!(&encoded[0..12], &[0u8; 12]);
        assert_eq!(&encoded[12..], addr.as_bytes());
    }

    #[test]
    fn test_uint256_encoding() {
        let value = U256::from(12345u64);
        let encoded = EIP712Verifier::encode_uint256(value);
        
        // Should be big-endian encoded
        let mut expected = [0u8; 32];
        expected[31] = 0x39;
        expected[30] = 0x30;
        assert_eq!(encoded, expected);
    }
}
