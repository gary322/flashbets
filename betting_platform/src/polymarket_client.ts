import * as WebSocket from 'ws';
import axios, { AxiosInstance } from 'axios';
import { RateLimiter } from 'limiter';
import { Market, PriceUpdate } from './types';

export class PolymarketClient {
    private rest: AxiosInstance;
    private ws: WebSocket | null = null;
    private limiter: RateLimiter;
    private reconnectAttempts = 0;

    constructor() {
        // REST client with retry logic
        const baseURL = process.env.POLYMARKET_API_URL || 'https://api.polymarket.com';
        this.rest = axios.create({
            baseURL,
            timeout: 10000,
            headers: {
                'User-Agent': 'BettingPlatform/1.0',
            },
        });

        // Rate limiter: 50 requests per 10 seconds (free tier)
        this.limiter = new RateLimiter({
            tokensPerInterval: 50,
            interval: 10000
        });

        // Add retry interceptor
        this.rest.interceptors.response.use(
            response => response,
            async error => {
                if (error.response?.status === 429) {
                    // Rate limited - exponential backoff
                    const delay = Math.pow(2, this.reconnectAttempts) * 1000;
                    await new Promise(resolve => setTimeout(resolve, delay));
                    this.reconnectAttempts++;
                    return this.rest.request(error.config);
                }
                throw error;
            }
        );
    }

    async fetchMarkets(limit = 1000, offset = 0): Promise<Market[]> {
        await this.limiter.removeTokens(1);

        const response = await this.rest.get('/markets', {
            params: { limit, offset, active: true }
        });

        return response.data.map((m: any) => ({
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
    }

    connectWebSocket(onMessage: (data: PriceUpdate) => void) {
        const wsUrl = process.env.POLYMARKET_WS_URL || 'wss://api.polymarket.com/ws';

        this.ws = new (WebSocket as any)(wsUrl);

        this.ws.on('open', () => {
            console.log('WebSocket connected to Polymarket');
            this.reconnectAttempts = 0;

            // Subscribe to all markets
            this.ws!.send(JSON.stringify({
                type: 'subscribe',
                channel: 'market_updates',
                params: { all: true }
            }));
        });

        this.ws.on('message', (data: string) => {
            try {
                const parsed = JSON.parse(data);
                if (parsed.type === 'price_update') {
                    onMessage({
                        marketId: parsed.market_id,
                        yesPrice: parseFloat(parsed.yes_price),
                        timestamp: Date.now(),
                    });
                }
            } catch (e) {
                console.error('WebSocket parse error:', e);
            }
        });

        this.ws.on('error', (error) => {
            console.error('WebSocket error:', error);
        });

        this.ws.on('close', () => {
            console.log('WebSocket disconnected, reconnecting...');
            setTimeout(() => this.connectWebSocket(onMessage),
                Math.pow(2, this.reconnectAttempts) * 1000
            );
            this.reconnectAttempts++;
        });
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }

    isConnected(): boolean {
        return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
    }
}