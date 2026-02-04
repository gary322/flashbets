import { EventEmitter } from 'events';
import { PriceUpdate } from '../../types';

interface WebSocketConfig {
  url: string;
  reconnectDelay: number;
  maxReconnectDelay: number;
  heartbeatInterval: number;
}

export class PolymarketWebSocket extends EventEmitter {
  private ws: WebSocket | null = null;
  private config: WebSocketConfig;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private heartbeatTimer: NodeJS.Timeout | null = null;
  private reconnectAttempts = 0;
  private subscriptions = new Set<string>();
  private lastPrices = new Map<string, PriceUpdate>();

  constructor(config: WebSocketConfig) {
    super();
    this.config = config;
  }

  connect(): void {
    try {
      this.ws = new WebSocket(this.config.url);

      this.ws.onopen = () => {
        console.log('WebSocket connected to Polymarket');
        this.reconnectAttempts = 0;
        this.emit('connected');

        // Resubscribe to all markets
        this.subscriptions.forEach(marketId => {
          this.subscribe(marketId);
        });

        // Start heartbeat
        this.startHeartbeat();
      };

      this.ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          this.handleMessage(data);
        } catch (error) {
          console.error('Failed to parse WebSocket message:', error);
        }
      };

      this.ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        this.emit('error', error);
      };

      this.ws.onclose = () => {
        console.log('WebSocket disconnected');
        this.emit('disconnected');
        this.stopHeartbeat();
        this.scheduleReconnect();
      };
    } catch (error) {
      console.error('Failed to create WebSocket:', error);
      this.scheduleReconnect();
    }
  }

  private handleMessage(data: any): void {
    switch (data.type) {
      case 'price_update':
        this.handlePriceUpdate(data.payload);
        break;

      case 'orderbook_update':
        this.emit('orderbook', data.payload);
        break;

      case 'trade':
        this.emit('trade', data.payload);
        break;

      case 'heartbeat':
        // Reset heartbeat timer
        this.resetHeartbeat();
        break;

      default:
        console.warn('Unknown message type:', data.type);
    }
  }

  private handlePriceUpdate(update: PriceUpdate): void {
    const lastPrice = this.lastPrices.get(update.marketId);

    // Check for stale prices (>60s)
    if (lastPrice && update.timestamp - lastPrice.timestamp > 60000) {
      this.emit('stale_price', {
        marketId: update.marketId,
        lastUpdate: lastPrice.timestamp
      });
    }

    // Store update
    this.lastPrices.set(update.marketId, update);

    // Emit price update
    this.emit('price', update);

    // Check for significant moves
    if (lastPrice && Math.abs(update.changePercent) > 5) {
      this.emit('significant_move', {
        marketId: update.marketId,
        change: update.changePercent
      });
    }
  }

  subscribe(marketId: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      this.subscriptions.add(marketId);
      return;
    }

    this.ws.send(JSON.stringify({
      type: 'subscribe',
      channel: 'market',
      marketId
    }));

    this.subscriptions.add(marketId);
  }

  unsubscribe(marketId: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      this.subscriptions.delete(marketId);
      return;
    }

    this.ws.send(JSON.stringify({
      type: 'unsubscribe',
      channel: 'market',
      marketId
    }));

    this.subscriptions.delete(marketId);
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, this.config.heartbeatInterval);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private resetHeartbeat(): void {
    this.stopHeartbeat();
    this.startHeartbeat();
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;

    const delay = Math.min(
      this.config.reconnectDelay * Math.pow(2, this.reconnectAttempts),
      this.config.maxReconnectDelay
    );

    this.reconnectAttempts++;

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    this.stopHeartbeat();

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    this.subscriptions.clear();
    this.lastPrices.clear();
  }

  getLastPrice(marketId: string): PriceUpdate | undefined {
    return this.lastPrices.get(marketId);
  }

  isStale(marketId: string, maxAge = 60000): boolean {
    const price = this.lastPrices.get(marketId);
    if (!price) return true;

    return Date.now() - price.timestamp > maxAge;
  }
}