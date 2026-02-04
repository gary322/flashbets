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
Object.defineProperty(exports, "__esModule", { value: true });
const globals_1 = require("@jest/globals");
const WebSocket = __importStar(require("ws"));
const verse_classifier_1 = require("../src/verse_classifier");
const polymarket_client_1 = require("../src/polymarket_client");
const mock_polymarket_1 = require("../src/mock_polymarket");
(0, globals_1.describe)('Verse Classification Tests', () => {
    const classifier = new verse_classifier_1.VerseClassifier();
    (0, globals_1.it)('should classify similar markets to the same verse', () => {
        const id1 = classifier.classifyMarket("BTC > $150k");
        const id2 = classifier.classifyMarket("Bitcoin above $150,000");
        const id3 = classifier.classifyMarket("Will BTC exceed $150000?");
        (0, globals_1.expect)(id1).toBe(id2);
        (0, globals_1.expect)(id1).toBe(id3);
    });
    (0, globals_1.it)('should classify different markets to different verses', () => {
        const btcId = classifier.classifyMarket("BTC > $150k");
        const ethId = classifier.classifyMarket("ETH > $10k");
        const electionId = classifier.classifyMarket("Will Alice win the election?");
        (0, globals_1.expect)(btcId).not.toBe(ethId);
        (0, globals_1.expect)(btcId).not.toBe(electionId);
        (0, globals_1.expect)(ethId).not.toBe(electionId);
    });
    (0, globals_1.it)('should handle normalization correctly', () => {
        const id1 = classifier.classifyMarket("BTC above $100,000 by December");
        const id2 = classifier.classifyMarket("btc > 100000 usd december");
        (0, globals_1.expect)(id1).toBe(id2);
    });
    (0, globals_1.it)('should calculate Levenshtein distance correctly', () => {
        const dist1 = verse_classifier_1.VerseClassifier.calculateLevenshteinDistance("kitten", "sitting");
        (0, globals_1.expect)(dist1).toBe(3);
        const dist2 = verse_classifier_1.VerseClassifier.calculateLevenshteinDistance("saturday", "sunday");
        (0, globals_1.expect)(dist2).toBe(3);
        const dist3 = verse_classifier_1.VerseClassifier.calculateLevenshteinDistance("", "abc");
        (0, globals_1.expect)(dist3).toBe(3);
        const dist4 = verse_classifier_1.VerseClassifier.calculateLevenshteinDistance("same", "same");
        (0, globals_1.expect)(dist4).toBe(0);
    });
});
(0, globals_1.describe)('Weighted Probability Calculations', () => {
    (0, globals_1.it)('should calculate weighted average correctly', () => {
        const markets = [
            { yes_price: 0.6, volume: 1000, liquidity: 500 },
            { yes_price: 0.7, volume: 2000, liquidity: 1000 },
            { yes_price: 0.65, volume: 500, liquidity: 250 },
        ];
        let totalWeight = 0;
        let weightedSum = 0;
        for (const market of markets) {
            const weight = market.volume * market.liquidity;
            totalWeight += weight;
            weightedSum += market.yes_price * weight;
        }
        const derivedProb = weightedSum / totalWeight;
        // Expected: (0.6*500000 + 0.7*2000000 + 0.65*125000) / 2625000 ≈ 0.6571
        (0, globals_1.expect)(derivedProb).toBeCloseTo(0.6571, 4);
    });
    (0, globals_1.it)('should handle zero weights correctly', () => {
        const markets = [
            { yes_price: 0.6, volume: 0, liquidity: 500 },
            { yes_price: 0.7, volume: 1000, liquidity: 0 },
            { yes_price: 0.8, volume: 1000, liquidity: 1000 },
        ];
        let totalWeight = 0;
        let weightedSum = 0;
        for (const market of markets) {
            const weight = market.volume * market.liquidity;
            totalWeight += weight;
            weightedSum += market.yes_price * weight;
        }
        const derivedProb = totalWeight > 0 ? weightedSum / totalWeight : 0.5;
        // Only the third market has non-zero weight
        (0, globals_1.expect)(derivedProb).toBe(0.8);
    });
    (0, globals_1.it)('should default to 0.5 when all weights are zero', () => {
        const markets = [
            { yes_price: 0.6, volume: 0, liquidity: 0 },
            { yes_price: 0.7, volume: 0, liquidity: 0 },
        ];
        let totalWeight = 0;
        let weightedSum = 0;
        for (const market of markets) {
            const weight = market.volume * market.liquidity;
            totalWeight += weight;
            weightedSum += market.yes_price * weight;
        }
        const derivedProb = totalWeight > 0 ? weightedSum / totalWeight : 0.5;
        (0, globals_1.expect)(derivedProb).toBe(0.5);
    });
});
(0, globals_1.describe)('Polymarket Client Integration', () => {
    let mockServer;
    let client;
    (0, globals_1.beforeAll)(() => {
        mockServer = new mock_polymarket_1.MockPolymarketServer(3001, 3002);
        // Point client to mock server
        process.env.POLYMARKET_API_URL = 'http://localhost:3001';
        process.env.POLYMARKET_WS_URL = 'ws://localhost:3002';
        client = new polymarket_client_1.PolymarketClient();
    });
    (0, globals_1.afterAll)(() => {
        mockServer.stop();
        client.disconnect();
    });
    (0, globals_1.it)('should fetch markets from API', () => __awaiter(void 0, void 0, void 0, function* () {
        const markets = yield client.fetchMarkets(10, 0);
        (0, globals_1.expect)(markets).toHaveLength(10);
        (0, globals_1.expect)(markets[0]).toHaveProperty('id');
        (0, globals_1.expect)(markets[0]).toHaveProperty('question');
        (0, globals_1.expect)(markets[0]).toHaveProperty('yes_price');
    }));
    (0, globals_1.it)('should handle pagination correctly', () => __awaiter(void 0, void 0, void 0, function* () {
        const batch1 = yield client.fetchMarkets(5, 0);
        const batch2 = yield client.fetchMarkets(5, 5);
        (0, globals_1.expect)(batch1).toHaveLength(5);
        (0, globals_1.expect)(batch2).toHaveLength(5);
        (0, globals_1.expect)(batch1[0].id).not.toBe(batch2[0].id);
    }));
    (0, globals_1.it)('should receive WebSocket price updates', (done) => {
        const updates = [];
        client.connectWebSocket((update) => {
            updates.push(update);
            if (updates.length >= 5) {
                (0, globals_1.expect)(updates.length).toBeGreaterThanOrEqual(5);
                (0, globals_1.expect)(updates[0]).toHaveProperty('marketId');
                (0, globals_1.expect)(updates[0]).toHaveProperty('yesPrice');
                (0, globals_1.expect)(updates[0]).toHaveProperty('timestamp');
                done();
            }
        });
    });
    (0, globals_1.it)('should respect rate limits', () => __awaiter(void 0, void 0, void 0, function* () {
        const startTime = Date.now();
        const requests = [];
        // Try to make 60 requests (should hit rate limit)
        for (let i = 0; i < 60; i++) {
            requests.push(client.fetchMarkets(1, i));
        }
        yield Promise.all(requests);
        const duration = Date.now() - startTime;
        // Should take more than 10 seconds due to rate limiting (50 requests per 10s)
        (0, globals_1.expect)(duration).toBeGreaterThan(10000);
    }));
});
(0, globals_1.describe)('Market Classification Integration', () => {
    let mockServer;
    let classifier;
    (0, globals_1.beforeAll)(() => {
        mockServer = new mock_polymarket_1.MockPolymarketServer(3001, 3002);
        classifier = new verse_classifier_1.VerseClassifier();
    });
    (0, globals_1.afterAll)(() => {
        mockServer.stop();
    });
    (0, globals_1.it)('should classify mock markets into verses', () => {
        const marketQuestions = [
            "Will BTC be above $100000 by 2025-12-31?",
            "Will Bitcoin exceed $100k before December?",
            "Will ETH reach $10000 before 2025-12-31?",
            "Will Ethereum hit $10k by year end?",
        ];
        const verses = new Map();
        for (const question of marketQuestions) {
            const verseId = classifier.classifyMarket(question);
            if (!verses.has(verseId)) {
                verses.set(verseId, []);
            }
            verses.get(verseId).push(question);
        }
        // Should have 2 verses (BTC and ETH)
        (0, globals_1.expect)(verses.size).toBe(2);
        // Each verse should have 2 similar questions
        for (const [verseId, questions] of verses) {
            (0, globals_1.expect)(questions.length).toBe(2);
        }
    });
});
(0, globals_1.describe)('Resolution and Dispute Flow', () => {
    let mockServer;
    (0, globals_1.beforeAll)(() => {
        mockServer = new mock_polymarket_1.MockPolymarketServer(3001, 3002);
    });
    (0, globals_1.afterAll)(() => {
        mockServer.stop();
    });
    (0, globals_1.it)('should handle market resolution', () => __awaiter(void 0, void 0, void 0, function* () {
        const market = mockServer.getMarket('market_0');
        (0, globals_1.expect)(market === null || market === void 0 ? void 0 : market.resolved).toBe(false);
        // Simulate resolution
        const response = yield fetch('http://localhost:3001/markets/market_0/resolve', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ resolution: 'Yes' }),
        });
        const resolvedMarket = yield response.json();
        (0, globals_1.expect)(resolvedMarket.resolved).toBe(true);
        (0, globals_1.expect)(resolvedMarket.resolution).toBe('Yes');
    }));
    (0, globals_1.it)('should broadcast resolution updates via WebSocket', (done) => {
        const ws = new WebSocket('ws://localhost:3002');
        ws.on('open', () => {
            ws.send(JSON.stringify({
                type: 'subscribe',
                channel: 'market_updates',
            }));
        });
        ws.on('message', (data) => {
            const message = JSON.parse(data);
            if (message.type === 'resolution_update') {
                (0, globals_1.expect)(message).toHaveProperty('market_id');
                (0, globals_1.expect)(message).toHaveProperty('resolution');
                ws.close();
                done();
            }
        });
        // Trigger a mass resolution after connection
        setTimeout(() => {
            mockServer.simulateMassResolution(1);
        }, 100);
    });
});
