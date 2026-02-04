// API Client for connecting to the real backend

const API_BASE_URL = 'http://localhost:8081';
const WS_URL = 'ws://localhost:8081/ws';

class BettingPlatformAPI {
    constructor() {
        this.baseURL = API_BASE_URL;
        this.ws = null;
        this.wsCallbacks = new Map();
    }

    // Initialize WebSocket connection
    initWebSocket() {
        this.ws = new WebSocket(WS_URL);
        
        this.ws.onopen = () => {
            console.log('WebSocket connected');
            this.emit('connected');
        };
        
        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                this.handleWebSocketMessage(data);
            } catch (err) {
                console.error('Failed to parse WebSocket message:', err);
            }
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.emit('error', error);
        };
        
        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            this.emit('disconnected');
            // Reconnect after 5 seconds
            setTimeout(() => this.initWebSocket(), 5000);
        };
    }

    handleWebSocketMessage(data) {
        switch (data.type) {
            case 'MarketUpdate':
                this.emit('marketUpdate', data);
                break;
            case 'PositionUpdate':
                this.emit('positionUpdate', data);
                break;
            case 'Notification':
                this.emit('notification', data);
                break;
        }
    }

    on(event, callback) {
        if (!this.wsCallbacks.has(event)) {
            this.wsCallbacks.set(event, []);
        }
        this.wsCallbacks.get(event).push(callback);
    }

    emit(event, data) {
        const callbacks = this.wsCallbacks.get(event) || [];
        callbacks.forEach(cb => cb(data));
    }

    // API Methods
    async request(endpoint, options = {}) {
        const response = await fetch(`${this.baseURL}${endpoint}`, {
            ...options,
            headers: {
                'Content-Type': 'application/json',
                ...options.headers,
            },
        });

        if (!response.ok) {
            throw new Error(`API request failed: ${response.statusText}`);
        }

        return response.json();
    }

    // Market APIs
    async getMarkets() {
        return this.request('/api/markets');
    }

    async getMarket(marketId) {
        return this.request(`/api/markets/${marketId}`);
    }

    async createMarket(marketData) {
        return this.request('/api/markets/create', {
            method: 'POST',
            body: JSON.stringify(marketData),
        });
    }

    // Trading APIs
    async placeTrade(tradeData) {
        return this.request('/api/trade/place', {
            method: 'POST',
            body: JSON.stringify(tradeData),
        });
    }

    async closePosition(positionId) {
        return this.request('/api/trade/close', {
            method: 'POST',
            body: JSON.stringify({ position_id: positionId }),
        });
    }

    // Position APIs
    async getPositions(wallet) {
        return this.request(`/api/positions/${wallet}`);
    }

    async getPortfolio(wallet) {
        return this.request(`/api/portfolio/${wallet}`);
    }

    // Wallet APIs
    async getBalance(wallet) {
        return this.request(`/api/wallet/balance/${wallet}`);
    }

    async createDemoAccount() {
        return this.request('/api/wallet/demo/create', {
            method: 'POST',
            body: JSON.stringify({}),
        });
    }

    // Verse APIs
    async getVerses() {
        return this.request('/api/verses');
    }

    async getVerse(verseId) {
        return this.request(`/api/verses/${verseId}`);
    }

    // Quantum APIs
    async getQuantumPositions(wallet) {
        return this.request(`/api/quantum/positions/${wallet}`);
    }

    async createQuantumPosition(data) {
        return this.request('/api/quantum/create', {
            method: 'POST',
            body: JSON.stringify(data),
        });
    }

    // DeFi APIs
    async stakeMmt(amount) {
        return this.request('/api/defi/stake', {
            method: 'POST',
            body: JSON.stringify({ amount }),
        });
    }

    async getLiquidityPools() {
        return this.request('/api/defi/pools');
    }
}

// Create global instance
window.bettingAPI = new BettingPlatformAPI();