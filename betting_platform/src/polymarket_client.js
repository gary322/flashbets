"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.PolymarketClient = void 0;
const WebSocket = __importStar(require("ws"));
const axios_1 = __importDefault(require("axios"));
const limiter_1 = require("limiter");
class PolymarketClient {
    constructor() {
        this.ws = null;
        this.reconnectAttempts = 0;
        // REST client with retry logic
        const baseURL = process.env.POLYMARKET_API_URL || 'https://api.polymarket.com';
        this.rest = axios_1.default.create({
            baseURL,
            timeout: 10000,
            headers: {
                'User-Agent': 'BettingPlatform/1.0',
            },
        });
        // Rate limiter: 50 requests per 10 seconds (free tier)
        this.limiter = new limiter_1.RateLimiter({
            tokensPerInterval: 50,
            interval: 10000
        });
        // Add retry interceptor
        this.rest.interceptors.response.use(response => response, (error) => __awaiter(this, void 0, void 0, function* () {
            var _a;
            if (((_a = error.response) === null || _a === void 0 ? void 0 : _a.status) === 429) {
                // Rate limited - exponential backoff
                const delay = Math.pow(2, this.reconnectAttempts) * 1000;
                yield new Promise(resolve => setTimeout(resolve, delay));
                this.reconnectAttempts++;
                return this.rest.request(error.config);
            }
            throw error;
        }));
    }
    fetchMarkets() {
        return __awaiter(this, arguments, void 0, function* (limit = 1000, offset = 0) {
            yield this.limiter.removeTokens(1);
            const response = yield this.rest.get('/markets', {
                params: { limit, offset, active: true }
            });
            return response.data.map((m) => ({
                id: m.id,
                question: m.question,
                outcomes: m.outcomes,
                volume: parseFloat(m.volume),
                liquidity: parseFloat(m.liquidity),
                yes_price: parseFloat(m.yes_price), // Current probability
                last_price: parseFloat(m.last_price),
                resolved: m.resolved,
                resolution: m.resolution,
                created_at: m.created_at,
                updated_at: m.updated_at,
            }));
        });
    }
    connectWebSocket(onMessage) {
        const wsUrl = process.env.POLYMARKET_WS_URL || 'wss://api.polymarket.com/ws';
        this.ws = new WebSocket(wsUrl);
        this.ws.on('open', () => {
            console.log('WebSocket connected to Polymarket');
            this.reconnectAttempts = 0;
            // Subscribe to all markets
            this.ws.send(JSON.stringify({
                type: 'subscribe',
                channel: 'market_updates',
                params: { all: true }
            }));
        });
        this.ws.on('message', (data) => {
            try {
                const parsed = JSON.parse(data);
                if (parsed.type === 'price_update') {
                    onMessage({
                        marketId: parsed.market_id,
                        yesPrice: parseFloat(parsed.yes_price),
                        timestamp: Date.now(),
                    });
                }
            }
            catch (e) {
                console.error('WebSocket parse error:', e);
            }
        });
        this.ws.on('error', (error) => {
            console.error('WebSocket error:', error);
        });
        this.ws.on('close', () => {
            console.log('WebSocket disconnected, reconnecting...');
            setTimeout(() => this.connectWebSocket(onMessage), Math.pow(2, this.reconnectAttempts) * 1000);
            this.reconnectAttempts++;
        });
    }
    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }
    isConnected() {
        return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
    }
}
exports.PolymarketClient = PolymarketClient;
