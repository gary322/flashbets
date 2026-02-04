"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PolymarketWebSocket = void 0;
const events_1 = require("events");
class PolymarketWebSocket extends events_1.EventEmitter {
    constructor(config) {
        super();
        this.ws = null;
        this.reconnectTimer = null;
        this.heartbeatTimer = null;
        this.reconnectAttempts = 0;
        this.subscriptions = new Set();
        this.lastPrices = new Map();
        this.config = config;
    }
    connect() {
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
                }
                catch (error) {
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
        }
        catch (error) {
            console.error('Failed to create WebSocket:', error);
            this.scheduleReconnect();
        }
    }
    handleMessage(data) {
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
    handlePriceUpdate(update) {
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
    subscribe(marketId) {
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
    unsubscribe(marketId) {
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
    startHeartbeat() {
        this.heartbeatTimer = setInterval(() => {
            if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                this.ws.send(JSON.stringify({ type: 'ping' }));
            }
        }, this.config.heartbeatInterval);
    }
    stopHeartbeat() {
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = null;
        }
    }
    resetHeartbeat() {
        this.stopHeartbeat();
        this.startHeartbeat();
    }
    scheduleReconnect() {
        if (this.reconnectTimer)
            return;
        const delay = Math.min(this.config.reconnectDelay * Math.pow(2, this.reconnectAttempts), this.config.maxReconnectDelay);
        this.reconnectAttempts++;
        this.reconnectTimer = setTimeout(() => {
            this.reconnectTimer = null;
            this.connect();
        }, delay);
    }
    disconnect() {
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
    getLastPrice(marketId) {
        return this.lastPrices.get(marketId);
    }
    isStale(marketId, maxAge = 60000) {
        const price = this.lastPrices.get(marketId);
        if (!price)
            return true;
        return Date.now() - price.timestamp > maxAge;
    }
}
exports.PolymarketWebSocket = PolymarketWebSocket;
