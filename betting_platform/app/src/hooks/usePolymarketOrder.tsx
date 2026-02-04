import { useState, useCallback } from 'react';
import { useMetaMask } from './useMetaMask';
import { ethers } from 'ethers';

// Polymarket CLOB addresses
const POLYMARKET_EXCHANGE_ADDRESS = '0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E'; // Exchange contract on Polygon
const POLYMARKET_CONDITIONAL_TOKENS_ADDRESS = '0x7D8610E9567d2a6C9FBB0A340e5d1dCc2cD5A9f4'; // CTF contract

// EIP-712 Domain
const DOMAIN = {
  name: 'Polymarket CLOB',
  version: '1',
  chainId: 137, // Polygon mainnet
  verifyingContract: POLYMARKET_EXCHANGE_ADDRESS
};

// EIP-712 Types
const TYPES = {
  Order: [
    { name: 'salt', type: 'uint256' },
    { name: 'maker', type: 'address' },
    { name: 'signer', type: 'address' },
    { name: 'taker', type: 'address' },
    { name: 'tokenId', type: 'uint256' },
    { name: 'makerAmount', type: 'uint256' },
    { name: 'takerAmount', type: 'uint256' },
    { name: 'expiration', type: 'uint256' },
    { name: 'nonce', type: 'uint256' },
    { name: 'feeRateBps', type: 'uint256' },
    { name: 'side', type: 'uint8' },
    { name: 'signatureType', type: 'uint8' }
  ]
};

export interface OrderParams {
  tokenId: string;
  side: 'buy' | 'sell';
  price: number; // Price in range 0-1
  size: number; // Size in USDC
  marketId: string;
  outcome: number;
}

export interface PreparedOrder {
  order: any;
  typedData: any;
  displayData: {
    market: string;
    side: string;
    outcome: string;
    price: string;
    size: string;
    fee: string;
    total: string;
  };
}

export function usePolymarketOrder() {
  const { wallet, signOrder, approveUSDC } = useMetaMask();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const prepareOrder = useCallback(async (params: OrderParams): Promise<PreparedOrder> => {
    if (!wallet.isConnected || !wallet.address) {
      throw new Error('Wallet not connected');
    }

    const { tokenId, side, price, size, outcome } = params;

    // Calculate amounts based on price and size
    // For buy orders: maker gives USDC, taker gives outcome tokens
    // For sell orders: maker gives outcome tokens, taker gives USDC
    const isBuy = side === 'buy';
    const priceInWei = ethers.utils.parseUnits(price.toFixed(6), 6); // USDC has 6 decimals
    const sizeInWei = ethers.utils.parseUnits(size.toFixed(6), 6);
    
    // Calculate maker and taker amounts
    let makerAmount: ethers.BigNumber;
    let takerAmount: ethers.BigNumber;
    
    if (isBuy) {
      // Buying: pay USDC, receive outcome tokens
      makerAmount = sizeInWei.mul(priceInWei).div(1e6); // USDC amount
      takerAmount = sizeInWei; // Outcome token amount
    } else {
      // Selling: pay outcome tokens, receive USDC
      makerAmount = sizeInWei; // Outcome token amount
      takerAmount = sizeInWei.mul(priceInWei).div(1e6); // USDC amount
    }

    // Generate order parameters
    const salt = ethers.BigNumber.from(ethers.utils.randomBytes(32)).toString();
    const nonce = Date.now(); // Simple nonce strategy
    const expiration = Math.floor(Date.now() / 1000) + 86400; // 24 hours from now
    const feeRateBps = 50; // 0.5% fee

    const order = {
      salt,
      maker: wallet.address,
      signer: wallet.address,
      taker: ethers.constants.AddressZero, // Any taker
      tokenId,
      makerAmount: makerAmount.toString(),
      takerAmount: takerAmount.toString(),
      expiration: expiration.toString(),
      nonce: nonce.toString(),
      feeRateBps: feeRateBps.toString(),
      side: isBuy ? 0 : 1, // 0 for buy, 1 for sell
      signatureType: 0 // EOA signature
    };

    // Prepare EIP-712 typed data
    const typedData = {
      domain: DOMAIN,
      types: TYPES,
      primaryType: 'Order',
      message: order
    };

    // Calculate display values
    const feeAmount = makerAmount.mul(feeRateBps).div(10000);
    const totalAmount = isBuy ? makerAmount.add(feeAmount) : makerAmount;

    const displayData = {
      market: params.marketId,
      side: side.toUpperCase(),
      outcome: `Outcome ${outcome}`,
      price: `${(price * 100).toFixed(1)}%`,
      size: `${size.toFixed(2)} USDC`,
      fee: `${ethers.utils.formatUnits(feeAmount, 6)} USDC`,
      total: `${ethers.utils.formatUnits(totalAmount, 6)} USDC`
    };

    return {
      order,
      typedData,
      displayData
    };
  }, [wallet]);

  const signAndSubmitOrder = useCallback(async (preparedOrder: PreparedOrder) => {
    setIsLoading(true);
    setError(null);

    try {
      // Step 1: Approve USDC if buying
      if (preparedOrder.order.side === 0) { // Buy order
        const totalAmount = ethers.BigNumber.from(preparedOrder.order.makerAmount)
          .mul(10050).div(10000); // Add 0.5% buffer for fees
        
        await approveUSDC(
          POLYMARKET_EXCHANGE_ADDRESS,
          ethers.utils.formatUnits(totalAmount, 6)
        );
      }

      // Step 2: Sign the order
      const signature = await signOrder(preparedOrder.typedData);

      // Step 3: Submit to backend
      const response = await fetch('/api/orders/submit', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          order: preparedOrder.order,
          signature,
          market_id: preparedOrder.displayData.market,
          marketId: preparedOrder.displayData.market
        })
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to submit order');
      }

      const result = await response.json();
      return result;

    } catch (err: any) {
      setError(err.message || 'Failed to sign and submit order');
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, [signOrder, approveUSDC]);

  return {
    prepareOrder,
    signAndSubmitOrder,
    isLoading,
    error
  };
}
