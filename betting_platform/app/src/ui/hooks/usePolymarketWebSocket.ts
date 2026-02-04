import { useEffect, useState, useRef, useCallback } from 'react';
import { PolymarketWebSocket } from '../services/websocket/PolymarketWebSocket';
import { PriceUpdate } from '../types';

interface UsePolymarketWebSocketReturn {
  prices: Map<string, PriceUpdate>;
  subscribe: (marketId: string) => void;
  unsubscribe: (marketId: string) => void;
  isConnected: boolean;
  isStale: (marketId: string) => boolean;
}

export const usePolymarketWebSocket = (): UsePolymarketWebSocketReturn => {
  const [prices, setPrices] = useState<Map<string, PriceUpdate>>(new Map());
  const [isConnected, setIsConnected] = useState(false);
  const wsRef = useRef<PolymarketWebSocket | null>(null);

  useEffect(() => {
    // Initialize WebSocket
    const ws = new PolymarketWebSocket({
      url: process.env.NEXT_PUBLIC_POLYMARKET_WS_URL || 'wss://api.polymarket.com/ws',
      reconnectDelay: 1000,
      maxReconnectDelay: 30000,
      heartbeatInterval: 30000
    });

    // Set up event listeners
    ws.on('connected', () => {
      setIsConnected(true);
    });

    ws.on('disconnected', () => {
      setIsConnected(false);
    });

    ws.on('price', (priceUpdate: PriceUpdate) => {
      setPrices(prev => {
        const newPrices = new Map(prev);
        newPrices.set(priceUpdate.marketId, priceUpdate);
        return newPrices;
      });
    });

    ws.on('error', (error) => {
      console.error('WebSocket error:', error);
    });

    ws.on('stale_price', ({ marketId, lastUpdate }) => {
      console.warn(`Stale price detected for market ${marketId}, last update: ${lastUpdate}`);
    });

    ws.on('significant_move', ({ marketId, change }) => {
      console.log(`Significant price move in market ${marketId}: ${change}%`);
    });

    // Connect
    ws.connect();
    wsRef.current = ws;

    // Cleanup
    return () => {
      ws.disconnect();
    };
  }, []);

  const subscribe = useCallback((marketId: string) => {
    if (wsRef.current) {
      wsRef.current.subscribe(marketId);
    }
  }, []);

  const unsubscribe = useCallback((marketId: string) => {
    if (wsRef.current) {
      wsRef.current.unsubscribe(marketId);
    }
  }, []);

  const isStale = useCallback((marketId: string) => {
    if (wsRef.current) {
      return wsRef.current.isStale(marketId);
    }
    return true;
  }, []);

  return {
    prices,
    subscribe,
    unsubscribe,
    isConnected,
    isStale
  };
};