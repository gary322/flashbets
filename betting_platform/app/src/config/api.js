"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.API_CONFIG = void 0;
exports.apiCall = apiCall;
// API Configuration
exports.API_CONFIG = {
    baseUrl: process.env.NEXT_PUBLIC_API_URL || '',
    wsUrl: process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:8081',
    endpoints: {
        health: '/health',
        markets: '/api/markets',
        market: (id) => `/api/markets/${id}`,
        verses: '/api/verses',
        verse: (id) => `/api/verses/${id}`,
        placeTrade: '/api/trade/place',
        positions: (wallet) => `/api/positions/${wallet}`,
        balance: (wallet) => `/api/wallet/balance/${wallet}`,
        createDemo: '/api/wallet/demo/create',
        walletChallenge: (wallet) => `/api/wallet/challenge/${wallet}`,
        walletVerify: '/api/wallet/verify',
    },
    websocket: {
        reconnectInterval: 5000,
        maxReconnectAttempts: 5,
    }
};
// Helper to make API calls
function apiCall(endpoint, options) {
    return __awaiter(this, void 0, void 0, function* () {
        var _a;
        const url = `${exports.API_CONFIG.baseUrl}${endpoint}`;
        try {
            const response = yield fetch(url, Object.assign(Object.assign({}, options), { headers: Object.assign({ 'Content-Type': 'application/json' }, options === null || options === void 0 ? void 0 : options.headers) }));
            if (!response.ok) {
                const error = yield response.json().catch(() => ({ error: 'Unknown error' }));
                throw new Error(((_a = error.error) === null || _a === void 0 ? void 0 : _a.message) || `API Error: ${response.status}`);
            }
            return yield response.json();
        }
        catch (error) {
            console.error('API call failed:', error);
            throw error;
        }
    });
}
