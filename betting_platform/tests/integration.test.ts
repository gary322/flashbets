import { describe, it, expect, beforeAll, afterAll } from '@jest/globals';
import * as WebSocket from 'ws';
import { VerseClassifier } from '../src/verse_classifier';
import { PolymarketClient } from '../src/polymarket_client';
import { MockPolymarketServer } from '../src/mock_polymarket';
import { Market } from '../src/types';

describe('Verse Classification Tests', () => {
    const classifier = new VerseClassifier();

    it('should classify similar markets to the same verse', () => {
        const id1 = classifier.classifyMarket("BTC > $150k");
        const id2 = classifier.classifyMarket("Bitcoin above $150,000");
        const id3 = classifier.classifyMarket("Will BTC exceed $150000?");
        
        expect(id1).toBe(id2);
        expect(id1).toBe(id3);
    });

    it('should classify different markets to different verses', () => {
        const btcId = classifier.classifyMarket("BTC > $150k");
        const ethId = classifier.classifyMarket("ETH > $10k");
        const electionId = classifier.classifyMarket("Will Alice win the election?");
        
        expect(btcId).not.toBe(ethId);
        expect(btcId).not.toBe(electionId);
        expect(ethId).not.toBe(electionId);
    });

    it('should handle normalization correctly', () => {
        const id1 = classifier.classifyMarket("BTC above $100,000 by December");
        const id2 = classifier.classifyMarket("btc > 100000 usd december");
        
        expect(id1).toBe(id2);
    });

    it('should calculate Levenshtein distance correctly', () => {
        const dist1 = VerseClassifier.calculateLevenshteinDistance("kitten", "sitting");
        expect(dist1).toBe(3);

        const dist2 = VerseClassifier.calculateLevenshteinDistance("saturday", "sunday");
        expect(dist2).toBe(3);

        const dist3 = VerseClassifier.calculateLevenshteinDistance("", "abc");
        expect(dist3).toBe(3);

        const dist4 = VerseClassifier.calculateLevenshteinDistance("same", "same");
        expect(dist4).toBe(0);
    });
});

describe('Weighted Probability Calculations', () => {
    it('should calculate weighted average correctly', () => {
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
        expect(derivedProb).toBeCloseTo(0.6571, 4);
    });

    it('should handle zero weights correctly', () => {
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
        expect(derivedProb).toBe(0.8);
    });

    it('should default to 0.5 when all weights are zero', () => {
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
        
        expect(derivedProb).toBe(0.5);
    });
});

describe('Polymarket Client Integration', () => {
    let mockServer: MockPolymarketServer;
    let client: PolymarketClient;

    beforeAll(() => {
        mockServer = new MockPolymarketServer(3001, 3002);
        // Point client to mock server
        process.env.POLYMARKET_API_URL = 'http://localhost:3001';
        process.env.POLYMARKET_WS_URL = 'ws://localhost:3002';
        client = new PolymarketClient();
    });

    afterAll(() => {
        mockServer.stop();
        client.disconnect();
    });

    it('should fetch markets from API', async () => {
        const markets = await client.fetchMarkets(10, 0);
        
        expect(markets).toHaveLength(10);
        expect(markets[0]).toHaveProperty('id');
        expect(markets[0]).toHaveProperty('question');
        expect(markets[0]).toHaveProperty('yes_price');
    });

    it('should handle pagination correctly', async () => {
        const batch1 = await client.fetchMarkets(5, 0);
        const batch2 = await client.fetchMarkets(5, 5);
        
        expect(batch1).toHaveLength(5);
        expect(batch2).toHaveLength(5);
        expect(batch1[0].id).not.toBe(batch2[0].id);
    });

    it('should receive WebSocket price updates', (done) => {
        const updates: any[] = [];
        
        client.connectWebSocket((update) => {
            updates.push(update);
            
            if (updates.length >= 5) {
                expect(updates.length).toBeGreaterThanOrEqual(5);
                expect(updates[0]).toHaveProperty('marketId');
                expect(updates[0]).toHaveProperty('yesPrice');
                expect(updates[0]).toHaveProperty('timestamp');
                done();
            }
        });
    });

    it('should respect rate limits', async () => {
        const startTime = Date.now();
        const requests = [];
        
        // Try to make 60 requests (should hit rate limit)
        for (let i = 0; i < 60; i++) {
            requests.push(client.fetchMarkets(1, i));
        }
        
        await Promise.all(requests);
        const duration = Date.now() - startTime;
        
        // Should take more than 10 seconds due to rate limiting (50 requests per 10s)
        expect(duration).toBeGreaterThan(10000);
    });
});

describe('Market Classification Integration', () => {
    let mockServer: MockPolymarketServer;
    let classifier: VerseClassifier;

    beforeAll(() => {
        mockServer = new MockPolymarketServer(3001, 3002);
        classifier = new VerseClassifier();
    });

    afterAll(() => {
        mockServer.stop();
    });

    it('should classify mock markets into verses', () => {
        const marketQuestions = [
            "Will BTC be above $100000 by 2025-12-31?",
            "Will Bitcoin exceed $100k before December?",
            "Will ETH reach $10000 before 2025-12-31?",
            "Will Ethereum hit $10k by year end?",
        ];

        const verses = new Map<string, string[]>();

        for (const question of marketQuestions) {
            const verseId = classifier.classifyMarket(question);
            if (!verses.has(verseId)) {
                verses.set(verseId, []);
            }
            verses.get(verseId)!.push(question);
        }

        // Should have 2 verses (BTC and ETH)
        expect(verses.size).toBe(2);

        // Each verse should have 2 similar questions
        for (const [verseId, questions] of verses) {
            expect(questions.length).toBe(2);
        }
    });
});

describe('Resolution and Dispute Flow', () => {
    let mockServer: MockPolymarketServer;

    beforeAll(() => {
        mockServer = new MockPolymarketServer(3001, 3002);
    });

    afterAll(() => {
        mockServer.stop();
    });

    it('should handle market resolution', async () => {
        const market = mockServer.getMarket('market_0');
        expect((market as any)?.resolved).toBe(false);

        // Simulate resolution
        const response = await fetch('http://localhost:3001/markets/market_0/resolve', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ resolution: 'Yes' }),
        });

        const resolvedMarket = await response.json() as any;
        expect(resolvedMarket.resolved).toBe(true);
        expect(resolvedMarket.resolution).toBe('Yes');
    });

    it('should broadcast resolution updates via WebSocket', (done) => {
        const ws = new (WebSocket as any)('ws://localhost:3002');
        
        ws.on('open', () => {
            ws.send(JSON.stringify({
                type: 'subscribe',
                channel: 'market_updates',
            }));
        });

        ws.on('message', (data: string) => {
            const message = JSON.parse(data);
            
            if (message.type === 'resolution_update') {
                expect(message).toHaveProperty('market_id');
                expect(message).toHaveProperty('resolution');
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