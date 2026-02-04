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
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.MockPolymarketServer = void 0;
exports.startMockPolymarketServer = startMockPolymarketServer;
const express_1 = __importDefault(require("express"));
const WebSocket = __importStar(require("ws"));
class MockPolymarketServer {
    constructor(port = 3001, wsPort = 3002) {
        this.markets = [];
        this.wsClients = new Set();
        this.generateMockMarkets();
        this.server = this.startMockAPI(port);
        this.wss = this.startMockWebSocket(wsPort);
        this.priceUpdateInterval = this.simulatePriceMovements();
    }
    generateMockMarkets() {
        // Generate 1000 mock markets
        for (let i = 0; i < 1000; i++) {
            this.markets.push({
                id: `market_${i}`,
                question: this.generateQuestion(i),
                outcomes: ['Yes', 'No'],
                yes_price: Math.random() * 0.8 + 0.1, // 0.1 to 0.9
                last_price: Math.random() * 0.8 + 0.1,
                volume: Math.random() * 1000000,
                liquidity: Math.random() * 500000,
                resolved: false,
                resolution: null,
                created_at: new Date(Date.now() - Math.random() * 30 * 24 * 60 * 60 * 1000).toISOString(),
                updated_at: new Date().toISOString(),
            });
        }
    }
    generateQuestion(index) {
        const templates = [
            "Will BTC be above $X by Y?",
            "Will ETH reach $X before Y?",
            "Will candidate X win the Y election?",
            "Will company X stock price exceed $Y by Z?",
            "Will team X win the Y championship?",
            "Will X be released before Y?",
            "Will the temperature in X exceed Y degrees on Z?",
            "Will X reach Y followers by Z?",
            "Will the GDP growth rate exceed X% in Y?",
            "Will X launch before Y date?"
        ];
        const template = templates[index % templates.length];
        const price = 50000 + index * 1000;
        const date = '2025-12-31';
        const candidate = ['Alice', 'Bob', 'Charlie'][index % 3];
        return template
            .replace('X', price.toString())
            .replace('Y', date)
            .replace('Z', '2025-06-30');
    }
    startMockAPI(port) {
        const app = (0, express_1.default)();
        app.use(express_1.default.json());
        // GET /markets endpoint
        app.get('/markets', (req, res) => {
            const limit = parseInt(req.query.limit) || 100;
            const offset = parseInt(req.query.offset) || 0;
            const active = req.query.active === 'true';
            let filteredMarkets = this.markets;
            if (active) {
                filteredMarkets = filteredMarkets.filter(m => !m.resolved);
            }
            const paginatedMarkets = filteredMarkets.slice(offset, offset + limit);
            res.json(paginatedMarkets);
        });
        // GET /markets/:id endpoint
        app.get('/markets/:id', (req, res) => {
            const market = this.markets.find(m => m.id === req.params.id);
            if (!market) {
                return res.status(404).json({ error: 'Market not found' });
            }
            res.json(market);
        });
        // Simulate resolution endpoint (for testing)
        app.post('/markets/:id/resolve', (req, res) => {
            const market = this.markets.find(m => m.id === req.params.id);
            if (!market) {
                return res.status(404).json({ error: 'Market not found' });
            }
            market.resolved = true;
            market.resolution = req.body.resolution || 'Yes';
            market.updated_at = new Date().toISOString();
            res.json(market);
        });
        // Simulate dispute endpoint
        app.post('/markets/:id/dispute', (req, res) => {
            const market = this.markets.find(m => m.id === req.params.id);
            if (!market) {
                return res.status(404).json({ error: 'Market not found' });
            }
            // Broadcast dispute to WebSocket clients
            const disputeMessage = {
                type: 'dispute_update',
                market_id: market.id,
                disputed: req.body.disputed,
            };
            this.broadcast(disputeMessage);
            res.json({ success: true });
        });
        const server = app.listen(port, () => {
            console.log(`Mock Polymarket API running on port ${port}`);
        });
        return server;
    }
    startMockWebSocket(port) {
        const wss = new WebSocket.Server({ port });
        wss.on('connection', (ws) => {
            console.log('New WebSocket client connected');
            this.wsClients.add(ws);
            ws.on('message', (data) => {
                try {
                    const message = JSON.parse(data);
                    if (message.type === 'subscribe' && message.channel === 'market_updates') {
                        // Client subscribed to market updates
                        ws.send(JSON.stringify({
                            type: 'subscription_success',
                            channel: 'market_updates'
                        }));
                    }
                }
                catch (e) {
                    console.error('WebSocket message parse error:', e);
                }
            });
            ws.on('close', () => {
                console.log('WebSocket client disconnected');
                this.wsClients.delete(ws);
            });
            ws.on('error', (error) => {
                console.error('WebSocket error:', error);
                this.wsClients.delete(ws);
            });
        });
        console.log(`Mock Polymarket WebSocket running on port ${port}`);
        return wss;
    }
    simulatePriceMovements() {
        return setInterval(() => {
            // Random price updates for 10 markets every 100ms
            for (let i = 0; i < 10; i++) {
                const market = this.markets[Math.floor(Math.random() * this.markets.length)];
                // Skip resolved markets
                if (market.resolved)
                    continue;
                // Simulate price movement (±5% max)
                const change = (Math.random() - 0.5) * 0.1;
                market.yes_price = Math.max(0.01, Math.min(0.99, market.yes_price + change));
                market.last_price = market.yes_price;
                market.updated_at = new Date().toISOString();
                // Broadcast to WebSocket clients
                const update = {
                    marketId: market.id,
                    yesPrice: market.yes_price,
                    timestamp: Date.now(),
                };
                this.broadcast({
                    type: 'price_update',
                    market_id: market.id,
                    yes_price: market.yes_price,
                });
            }
            // Occasionally resolve a market (1% chance per interval)
            if (Math.random() < 0.01) {
                const unresolvedMarkets = this.markets.filter(m => !m.resolved);
                if (unresolvedMarkets.length > 0) {
                    const marketToResolve = unresolvedMarkets[Math.floor(Math.random() * unresolvedMarkets.length)];
                    marketToResolve.resolved = true;
                    marketToResolve.resolution = Math.random() > 0.5 ? 'Yes' : 'No';
                    this.broadcast({
                        type: 'resolution_update',
                        market_id: marketToResolve.id,
                        resolution: marketToResolve.resolution,
                    });
                }
            }
        }, 100); // 10 updates per second
    }
    broadcast(message) {
        const data = JSON.stringify(message);
        this.wsClients.forEach(client => {
            if (client.readyState === WebSocket.OPEN) {
                client.send(data);
            }
        });
    }
    stop() {
        // Clean up resources
        if (this.priceUpdateInterval) {
            clearInterval(this.priceUpdateInterval);
        }
        // Close all WebSocket connections
        this.wsClients.forEach(client => {
            client.close();
        });
        // Close servers
        if (this.wss) {
            this.wss.close();
        }
        if (this.server) {
            this.server.close();
        }
        console.log('Mock Polymarket server stopped');
    }
    // Test helper methods
    getMarket(id) {
        return this.markets.find(m => m.id === id);
    }
    getMarketCount() {
        return this.markets.length;
    }
    getActiveMarketCount() {
        return this.markets.filter(m => !m.resolved).length;
    }
    simulateMassResolution(count) {
        const unresolvedMarkets = this.markets.filter(m => !m.resolved);
        const toResolve = unresolvedMarkets.slice(0, Math.min(count, unresolvedMarkets.length));
        toResolve.forEach(market => {
            market.resolved = true;
            market.resolution = Math.random() > 0.5 ? 'Yes' : 'No';
            this.broadcast({
                type: 'resolution_update',
                market_id: market.id,
                resolution: market.resolution,
            });
        });
        return toResolve.length;
    }
}
exports.MockPolymarketServer = MockPolymarketServer;
// Export a function to start the mock server
function startMockPolymarketServer(port = 3001, wsPort = 3002) {
    return new MockPolymarketServer(port, wsPort);
}
