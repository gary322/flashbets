"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.usePolymarketWebSocket = void 0;
const react_1 = require("react");
const PolymarketWebSocket_1 = require("../services/websocket/PolymarketWebSocket");
const usePolymarketWebSocket = () => {
    const [prices, setPrices] = (0, react_1.useState)(new Map());
    const [isConnected, setIsConnected] = (0, react_1.useState)(false);
    const wsRef = (0, react_1.useRef)(null);
    (0, react_1.useEffect)(() => {
        // Initialize WebSocket
        const ws = new PolymarketWebSocket_1.PolymarketWebSocket({
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
        ws.on('price', (priceUpdate) => {
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
    const subscribe = (0, react_1.useCallback)((marketId) => {
        if (wsRef.current) {
            wsRef.current.subscribe(marketId);
        }
    }, []);
    const unsubscribe = (0, react_1.useCallback)((marketId) => {
        if (wsRef.current) {
            wsRef.current.unsubscribe(marketId);
        }
    }, []);
    const isStale = (0, react_1.useCallback)((marketId) => {
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
exports.usePolymarketWebSocket = usePolymarketWebSocket;
