/**
 * Polymarket Service
 * Handles all interactions with Polymarket API endpoints
 */

import axios, { AxiosInstance } from 'axios';
import { ethers } from 'ethers';

// Types
export interface PolymarketOrder {
  salt: string;
  maker: string;
  signer: string;
  taker: string;
  tokenId: string;
  makerAmount: string;
  takerAmount: string;
  expiration: string;
  nonce: string;
  feeRateBps: string;
  side: number;
  signatureType: number;
}

export interface CreateOrderParams {
  marketId: string;
  conditionId: string;
  tokenId: string;
  outcome: number;
  side: 'buy' | 'sell';
  size: string;
  price: string;
  orderType?: 'gtc' | 'fok' | 'ioc';
  expiration?: number;
}

export interface OrderResponse {
  orderId: string;
  status: string;
  createdAt: string;
  size: string;
  price: string;
  filledAmount: string;
  remainingAmount: string;
  averageFillPrice?: string;
  estimatedFees: string;
}

export interface MarketData {
  conditionId: string;
  tokenId: string;
  liquidity: string;
  volume24h: string;
  lastPrice?: string;
  bid?: string;
  ask?: string;
  spread?: string;
  openInterest: string;
}

export interface Position {
  conditionId: string;
  outcomeIndex: number;
  balance: string;
  lockedBalance: string;
  averagePrice?: string;
  realizedPnl: string;
  unrealizedPnl: string;
  marketValue: string;
}

export interface Balance {
  usdcBalance: string;
  maticBalance: string;
  totalPositionValue: string;
  availableBalance: string;
  lockedInOrders: string;
}

export interface OrderBook {
  bids: OrderBookLevel[];
  asks: OrderBookLevel[];
  spread?: string;
  midPrice?: string;
}

export interface OrderBookLevel {
  price: string;
  size: string;
  numOrders: number;
}

export interface UserStats {
  totalVolumeTraded: string;
  totalMarketsTraded: number;
  winRate: number;
  totalPnl: string;
  bestTrade?: string;
  worstTrade?: string;
  activePositions: number;
  pendingOrders: number;
}

class PolymarketService {
  private api: AxiosInstance;
  private signer?: ethers.Signer;

  constructor(baseURL: string = '/api/polymarket') {
    this.api = axios.create({
      baseURL,
      headers: {
        'Content-Type': 'application/json',
      },
    });

    // Add auth token to requests
    this.api.interceptors.request.use((config) => {
      const token = localStorage.getItem('auth_token');
      if (token) {
        config.headers.Authorization = `Bearer ${token}`;
      }
      return config;
    });
  }

  /**
   * Set the signer for signing orders
   */
  setSigner(signer: ethers.Signer) {
    this.signer = signer;
  }

  // ==================== Order Management ====================

  /**
   * Create a new order (returns unsigned order for signing)
   */
  async createOrder(params: CreateOrderParams): Promise<PolymarketOrder> {
    const response = await this.api.post<{ data: PolymarketOrder }>('/orders', params);
    return response.data.data;
  }

  /**
   * Sign an order using EIP-712
   */
  async signOrder(order: PolymarketOrder): Promise<string> {
    if (!this.signer) {
      throw new Error('No signer configured');
    }

    const domain = {
      name: 'Polymarket',
      version: '1',
      chainId: 137, // Polygon mainnet
      verifyingContract: '0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E',
    };

    const types = {
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
        { name: 'signatureType', type: 'uint8' },
      ],
    };

    const signature = await this.signer._signTypedData(domain, types, order);
    return signature;
  }

  /**
   * Submit a signed order to Polymarket
   */
  async submitOrder(order: PolymarketOrder, signature: string): Promise<OrderResponse> {
    const response = await this.api.post<{ data: OrderResponse }>('/orders/submit', {
      orderData: order,
      signature,
    });
    return response.data.data;
  }

  /**
   * Create and submit an order in one step
   */
  async placeOrder(params: CreateOrderParams): Promise<OrderResponse> {
    // Create unsigned order
    const order = await this.createOrder(params);
    
    // Sign the order
    const signature = await this.signOrder(order);
    
    // Submit signed order
    return await this.submitOrder(order, signature);
  }

  /**
   * Cancel an order
   */
  async cancelOrder(orderId: string): Promise<void> {
    await this.api.delete(`/orders/${orderId}`);
  }

  /**
   * Get order status
   */
  async getOrder(orderId: string): Promise<OrderResponse> {
    const response = await this.api.get<{ data: OrderResponse }>(`/orders/${orderId}`);
    return response.data.data;
  }

  /**
   * Get user's orders
   */
  async getUserOrders(params?: {
    status?: string;
    marketId?: string;
    limit?: number;
    offset?: number;
  }): Promise<OrderResponse[]> {
    const response = await this.api.get<{ data: OrderResponse[] }>('/orders', { params });
    return response.data.data;
  }

  // ==================== Market Data ====================

  /**
   * Get market data
   */
  async getMarketData(conditionId: string): Promise<MarketData> {
    const response = await this.api.get<{ data: MarketData }>(`/markets/${conditionId}`);
    return response.data.data;
  }

  /**
   * Get order book
   */
  async getOrderBook(tokenId: string): Promise<OrderBook> {
    const response = await this.api.get<{ data: OrderBook }>(`/orderbook/${tokenId}`);
    return response.data.data;
  }

  /**
   * Get price history
   */
  async getPriceHistory(
    conditionId: string,
    hours: number = 24,
    resolution?: string
  ): Promise<Array<{ timestamp: string; price: string; volume?: string }>> {
    const response = await this.api.get<{ data: any[] }>(`/markets/${conditionId}/history`, {
      params: { hours, resolution },
    });
    return response.data.data;
  }

  /**
   * Sync market data from Polymarket
   */
  async syncMarket(conditionId: string): Promise<void> {
    await this.api.post(`/markets/${conditionId}/sync`);
  }

  // ==================== User Positions & Balances ====================

  /**
   * Get user's positions
   */
  async getPositions(): Promise<Position[]> {
    const response = await this.api.get<{ data: Position[] }>('/positions');
    return response.data.data;
  }

  /**
   * Get user's balances
   */
  async getBalances(): Promise<Balance> {
    const response = await this.api.get<{ data: Balance }>('/balances');
    return response.data.data;
  }

  /**
   * Get user statistics
   */
  async getUserStats(): Promise<UserStats> {
    const response = await this.api.get<{ data: UserStats }>('/stats');
    return response.data.data;
  }

  // ==================== CTF Operations ====================

  /**
   * Split position (mint outcome tokens)
   */
  async splitPosition(
    conditionId: string,
    amount: string
  ): Promise<{ txHash: string; yesTokens: string; noTokens: string; gasUsed: number }> {
    const response = await this.api.post<{ data: any }>('/ctf/split', {
      conditionId,
      amount,
    });
    return response.data.data;
  }

  /**
   * Merge positions (burn outcome tokens)
   */
  async mergePositions(
    conditionId: string,
    amount: string
  ): Promise<{ txHash: string; collateralReturned: string; gasUsed: number }> {
    const response = await this.api.post<{ data: any }>('/ctf/merge', {
      conditionId,
      amount,
    });
    return response.data.data;
  }

  /**
   * Redeem winning positions
   */
  async redeemPositions(
    conditionId: string,
    indexSets: string[]
  ): Promise<{ txHash: string; payout: string; gasUsed: number }> {
    const response = await this.api.post<{ data: any }>('/ctf/redeem', {
      conditionId,
      indexSets,
    });
    return response.data.data;
  }

  // ==================== WebSocket Connection ====================

  /**
   * Connect to Polymarket WebSocket for real-time updates
   */
  connectWebSocket(
    onMessage: (event: any) => void,
    onError?: (error: any) => void
  ): WebSocket {
    const ws = new WebSocket(process.env.REACT_APP_WS_URL || 'wss://api.example.com/ws');
    
    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        onMessage(data);
      } catch (error) {
        console.error('Failed to parse WebSocket message:', error);
      }
    };
    
    ws.onerror = (error) => {
      console.error('WebSocket error:', error);
      if (onError) onError(error);
    };
    
    ws.onopen = () => {
      console.log('WebSocket connected');
      // Subscribe to user's orders
      ws.send(JSON.stringify({
        type: 'subscribe',
        channel: 'orders',
      }));
    };
    
    return ws;
  }

  // ==================== Health Check ====================

  /**
   * Check Polymarket integration health
   */
  async healthCheck(): Promise<{
    clobConnected: boolean;
    websocketConnected: boolean;
    databaseConnected: boolean;
    lastSync?: string;
    pendingOrders: number;
    activePositions: number;
  }> {
    const response = await this.api.get<{ data: any }>('/health');
    return response.data.data;
  }
}

// Create singleton instance
const polymarketService = new PolymarketService();

export default polymarketService;