import { PolymarketWebSocket } from '../../services/websocket/PolymarketWebSocket';

// Mock WebSocket
class MockWebSocket {
  url: string;
  readyState: number = 0;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    setTimeout(() => {
      this.readyState = 1;
      if (this.onopen) {
        this.onopen(new Event('open'));
      }
    }, 10);
  }

  send(data: string) {
    // Mock send
  }

  close() {
    this.readyState = 3;
    if (this.onclose) {
      this.onclose(new CloseEvent('close'));
    }
  }

  simulateMessage(data: any) {
    if (this.onmessage) {
      this.onmessage(new MessageEvent('message', { data: JSON.stringify(data) }));
    }
  }

  simulateError() {
    if (this.onerror) {
      this.onerror(new Event('error'));
    }
  }
}

// Replace global WebSocket with mock
(global as any).WebSocket = MockWebSocket;

describe('PolymarketWebSocket', () => {
  let wsManager: PolymarketWebSocket;
  let mockWs: MockWebSocket;

  beforeEach(() => {
    jest.useFakeTimers();
    wsManager = new PolymarketWebSocket({
      url: 'ws://localhost:8080',
      reconnectDelay: 1000,
      maxReconnectDelay: 10000,
      heartbeatInterval: 30000
    });
  });

  afterEach(() => {
    wsManager.disconnect();
    jest.clearAllTimers();
    jest.useRealTimers();
  });

  describe('Connection Management', () => {
    it('should connect to WebSocket server', async () => {
      const connectedSpy = jest.fn();
      wsManager.on('connected', connectedSpy);

      wsManager.connect();
      
      // Wait for connection
      await jest.advanceTimersByTimeAsync(20);

      expect(connectedSpy).toHaveBeenCalled();
    });

    it('should emit disconnected event on close', async () => {
      const disconnectedSpy = jest.fn();
      wsManager.on('disconnected', disconnectedSpy);

      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);

      // Get the mock WebSocket instance
      mockWs = (wsManager as any).ws;
      mockWs.close();

      expect(disconnectedSpy).toHaveBeenCalled();
    });

    it('should handle connection errors', async () => {
      const errorSpy = jest.fn();
      wsManager.on('error', errorSpy);

      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);

      mockWs = (wsManager as any).ws;
      mockWs.simulateError();

      expect(errorSpy).toHaveBeenCalled();
    });

    it('should attempt reconnection on disconnect', async () => {
      const connectedSpy = jest.fn();
      wsManager.on('connected', connectedSpy);

      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);

      mockWs = (wsManager as any).ws;
      mockWs.close();

      // Wait for reconnect delay
      await jest.advanceTimersByTimeAsync(1000);

      expect(connectedSpy).toHaveBeenCalledTimes(2);
    });

    it('should use exponential backoff for reconnection', async () => {
      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);

      mockWs = (wsManager as any).ws;
      
      // First disconnect - 1000ms delay
      mockWs.close();
      await jest.advanceTimersByTimeAsync(999);
      expect((wsManager as any).ws).toBeNull();
      
      await jest.advanceTimersByTimeAsync(1);
      expect((wsManager as any).ws).not.toBeNull();

      // Second disconnect - 2000ms delay
      mockWs = (wsManager as any).ws;
      mockWs.close();
      await jest.advanceTimersByTimeAsync(1999);
      expect((wsManager as any).reconnectTimer).not.toBeNull();
      
      await jest.advanceTimersByTimeAsync(1);
      expect((wsManager as any).ws).not.toBeNull();
    });
  });

  describe('Message Handling', () => {
    beforeEach(async () => {
      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);
      mockWs = (wsManager as any).ws;
    });

    it('should handle price updates', () => {
      const priceSpy = jest.fn();
      wsManager.on('price', priceSpy);

      const priceUpdate = {
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.65,
          timestamp: Date.now(),
          volume24h: 1000000,
          changePercent: 2.5
        }
      };

      mockWs.simulateMessage(priceUpdate);

      expect(priceSpy).toHaveBeenCalledWith(priceUpdate.payload);
    });

    it('should detect stale prices', () => {
      const staleSpy = jest.fn();
      wsManager.on('stale_price', staleSpy);

      const oldTimestamp = Date.now() - 70000; // 70 seconds ago
      
      // First update to establish baseline
      mockWs.simulateMessage({
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.65,
          timestamp: oldTimestamp,
          volume24h: 1000000,
          changePercent: 0
        }
      });

      // Second update to trigger stale check
      mockWs.simulateMessage({
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.66,
          timestamp: Date.now(),
          volume24h: 1000000,
          changePercent: 1.5
        }
      });

      expect(staleSpy).toHaveBeenCalledWith({
        marketId: 'market1',
        lastUpdate: oldTimestamp
      });
    });

    it('should detect significant price moves', () => {
      const significantMoveSpy = jest.fn();
      wsManager.on('significant_move', significantMoveSpy);

      // First update
      mockWs.simulateMessage({
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.50,
          timestamp: Date.now(),
          volume24h: 1000000,
          changePercent: 0
        }
      });

      // Significant move
      mockWs.simulateMessage({
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.55,
          timestamp: Date.now(),
          volume24h: 1000000,
          changePercent: 10
        }
      });

      expect(significantMoveSpy).toHaveBeenCalledWith({
        marketId: 'market1',
        change: 10
      });
    });

    it('should handle orderbook updates', () => {
      const orderbookSpy = jest.fn();
      wsManager.on('orderbook', orderbookSpy);

      const orderbookUpdate = {
        type: 'orderbook_update',
        payload: {
          marketId: 'market1',
          bids: [[0.65, 1000]],
          asks: [[0.66, 2000]]
        }
      };

      mockWs.simulateMessage(orderbookUpdate);

      expect(orderbookSpy).toHaveBeenCalledWith(orderbookUpdate.payload);
    });

    it('should handle trade events', () => {
      const tradeSpy = jest.fn();
      wsManager.on('trade', tradeSpy);

      const tradeEvent = {
        type: 'trade',
        payload: {
          marketId: 'market1',
          price: 0.65,
          amount: 100,
          timestamp: Date.now()
        }
      };

      mockWs.simulateMessage(tradeEvent);

      expect(tradeSpy).toHaveBeenCalledWith(tradeEvent.payload);
    });

    it('should handle heartbeat messages', () => {
      const heartbeatSpy = jest.spyOn(wsManager as any, 'resetHeartbeat');

      mockWs.simulateMessage({ type: 'heartbeat' });

      expect(heartbeatSpy).toHaveBeenCalled();
    });

    it('should handle unknown message types gracefully', () => {
      const consoleSpy = jest.spyOn(console, 'warn').mockImplementation();

      mockWs.simulateMessage({ type: 'unknown_type', data: {} });

      expect(consoleSpy).toHaveBeenCalledWith('Unknown message type:', 'unknown_type');
      consoleSpy.mockRestore();
    });

    it('should handle malformed messages', () => {
      const consoleSpy = jest.spyOn(console, 'error').mockImplementation();

      if (mockWs.onmessage) {
        mockWs.onmessage(new MessageEvent('message', { data: 'invalid json' }));
      }

      expect(consoleSpy).toHaveBeenCalledWith('Failed to parse WebSocket message:', expect.any(Error));
      consoleSpy.mockRestore();
    });
  });

  describe('Subscription Management', () => {
    beforeEach(async () => {
      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);
      mockWs = (wsManager as any).ws;
    });

    it('should subscribe to markets', () => {
      const sendSpy = jest.spyOn(mockWs, 'send');

      wsManager.subscribe('market1');

      expect(sendSpy).toHaveBeenCalledWith(JSON.stringify({
        type: 'subscribe',
        channel: 'market',
        marketId: 'market1'
      }));
    });

    it('should unsubscribe from markets', () => {
      const sendSpy = jest.spyOn(mockWs, 'send');

      wsManager.subscribe('market1');
      wsManager.unsubscribe('market1');

      expect(sendSpy).toHaveBeenCalledWith(JSON.stringify({
        type: 'unsubscribe',
        channel: 'market',
        marketId: 'market1'
      }));
    });

    it('should queue subscriptions when not connected', () => {
      wsManager.disconnect();
      wsManager.subscribe('market1');

      expect((wsManager as any).subscriptions.has('market1')).toBe(true);
    });

    it('should resubscribe to markets on reconnection', async () => {
      wsManager.subscribe('market1');
      wsManager.subscribe('market2');

      mockWs.close();
      
      // Wait for reconnection
      await jest.advanceTimersByTimeAsync(1000);

      const newWs = (wsManager as any).ws;
      const sendSpy = jest.spyOn(newWs, 'send');

      await jest.advanceTimersByTimeAsync(20);

      expect(sendSpy).toHaveBeenCalledWith(JSON.stringify({
        type: 'subscribe',
        channel: 'market',
        marketId: 'market1'
      }));
      expect(sendSpy).toHaveBeenCalledWith(JSON.stringify({
        type: 'subscribe',
        channel: 'market',
        marketId: 'market2'
      }));
    });
  });

  describe('Heartbeat Mechanism', () => {
    beforeEach(async () => {
      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);
      mockWs = (wsManager as any).ws;
    });

    it('should send heartbeat pings', () => {
      const sendSpy = jest.spyOn(mockWs, 'send');

      // Advance to heartbeat interval
      jest.advanceTimersByTime(30000);

      expect(sendSpy).toHaveBeenCalledWith(JSON.stringify({ type: 'ping' }));
    });

    it('should stop heartbeat on disconnect', () => {
      const stopHeartbeatSpy = jest.spyOn(wsManager as any, 'stopHeartbeat');

      mockWs.close();

      expect(stopHeartbeatSpy).toHaveBeenCalled();
      expect((wsManager as any).heartbeatTimer).toBeNull();
    });
  });

  describe('Price Cache Management', () => {
    beforeEach(async () => {
      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);
      mockWs = (wsManager as any).ws;
    });

    it('should cache price updates', () => {
      const priceUpdate = {
        marketId: 'market1',
        price: 0.65,
        timestamp: Date.now(),
        volume24h: 1000000,
        changePercent: 2.5
      };

      mockWs.simulateMessage({
        type: 'price_update',
        payload: priceUpdate
      });

      const cachedPrice = wsManager.getLastPrice('market1');
      expect(cachedPrice).toEqual(priceUpdate);
    });

    it('should return undefined for uncached markets', () => {
      const price = wsManager.getLastPrice('unknown-market');
      expect(price).toBeUndefined();
    });

    it('should check if prices are stale', () => {
      const oldTimestamp = Date.now() - 120000; // 2 minutes ago

      mockWs.simulateMessage({
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.65,
          timestamp: oldTimestamp,
          volume24h: 1000000,
          changePercent: 0
        }
      });

      expect(wsManager.isStale('market1')).toBe(true);
      expect(wsManager.isStale('market1', 180000)).toBe(false); // 3 minute threshold
    });

    it('should consider non-existent markets as stale', () => {
      expect(wsManager.isStale('unknown-market')).toBe(true);
    });

    it('should clear cache on disconnect', () => {
      mockWs.simulateMessage({
        type: 'price_update',
        payload: {
          marketId: 'market1',
          price: 0.65,
          timestamp: Date.now(),
          volume24h: 1000000,
          changePercent: 0
        }
      });

      wsManager.disconnect();

      expect(wsManager.getLastPrice('market1')).toBeUndefined();
    });
  });

  describe('Error Handling', () => {
    it('should handle connection failure gracefully', () => {
      // Mock WebSocket to throw on construction
      const originalWebSocket = (global as any).WebSocket;
      (global as any).WebSocket = class {
        constructor() {
          throw new Error('Connection failed');
        }
      };

      const consoleSpy = jest.spyOn(console, 'error').mockImplementation();
      
      wsManager.connect();

      expect(consoleSpy).toHaveBeenCalledWith('Failed to create WebSocket:', expect.any(Error));
      
      consoleSpy.mockRestore();
      (global as any).WebSocket = originalWebSocket;
    });

    it('should clear reconnect timer on disconnect', async () => {
      wsManager.connect();
      await jest.advanceTimersByTimeAsync(20);

      mockWs = (wsManager as any).ws;
      mockWs.close();

      // Disconnect during reconnect attempt
      wsManager.disconnect();

      expect((wsManager as any).reconnectTimer).toBeNull();
    });
  });
});