/**
 * Market Data Module
 * Fetches and normalizes market data from Polymarket and Kalshi
 * Integrates with the data normalizer from the Rust codebase
 */

// API endpoints
const POLYMARKET_API = 'https://clob.polymarket.com/api';
const KALSHI_API = 'https://api.kalshi.com/v2';

// Cache duration (5 minutes)
const CACHE_DURATION = 5 * 60 * 1000;

// Market sources
export const MarketSource = {
    POLYMARKET: 0,
    KALSHI: 1,
    INTERNAL: 2
};

// Market status
export const MarketStatus = {
    ACTIVE: 0,
    CLOSED: 1,
    RESOLVED: 2,
    DISPUTED: 3,
    CANCELLED: 4,
    PAUSED: 5,
    INVALID: 6
};

class MarketDataService {
    constructor() {
        this.cache = new Map();
        this.subscriptions = new Map();
        this.wsConnections = new Map();
    }

    /**
     * Fetch market data from URL
     */
    async fetchMarketFromUrl(url) {
        try {
            // Determine source from URL
            const source = this.getSourceFromUrl(url);
            const marketId = this.extractMarketId(url);

            if (source === MarketSource.POLYMARKET) {
                return await this.fetchPolymarketData(marketId);
            } else if (source === MarketSource.KALSHI) {
                return await this.fetchKalshiData(marketId);
            } else {
                throw new Error('Unsupported market URL');
            }
        } catch (error) {
            console.error('Error fetching market:', error);
            throw error;
        }
    }

    /**
     * Get source from URL
     */
    getSourceFromUrl(url) {
        if (url.includes('polymarket.com')) {
            return MarketSource.POLYMARKET;
        } else if (url.includes('kalshi.com')) {
            return MarketSource.KALSHI;
        }
        return null;
    }

    /**
     * Extract market ID from URL
     */
    extractMarketId(url) {
        // Polymarket: https://polymarket.com/event/market-id
        // Kalshi: https://kalshi.com/markets/MARKET-ID
        const urlParts = url.split('/');
        return urlParts[urlParts.length - 1];
    }

    /**
     * Fetch Polymarket data
     */
    async fetchPolymarketData(marketId) {
        // Check cache first
        const cacheKey = `polymarket_${marketId}`;
        const cached = this.getFromCache(cacheKey);
        if (cached) return cached;

        try {
            // Fetch market data
            const response = await fetch(`${POLYMARKET_API}/markets/${marketId}`);
            if (!response.ok) {
                throw new Error(`Polymarket API error: ${response.status}`);
            }

            const data = await response.json();
            
            // Normalize to internal format
            const normalized = this.normalizePolymarketData(data);
            
            // Cache the result
            this.setCache(cacheKey, normalized);
            
            return normalized;
        } catch (error) {
            console.error('Polymarket fetch error:', error);
            throw error;
        }
    }

    /**
     * Fetch Kalshi data
     */
    async fetchKalshiData(marketId) {
        // Check cache first
        const cacheKey = `kalshi_${marketId}`;
        const cached = this.getFromCache(cacheKey);
        if (cached) return cached;

        try {
            // Fetch market data
            const response = await fetch(`${KALSHI_API}/markets/${marketId}`, {
                headers: {
                    'Accept': 'application/json'
                }
            });
            
            if (!response.ok) {
                throw new Error(`Kalshi API error: ${response.status}`);
            }

            const data = await response.json();
            
            // Normalize to internal format
            const normalized = this.normalizeKalshiData(data);
            
            // Cache the result
            this.setCache(cacheKey, normalized);
            
            return normalized;
        } catch (error) {
            console.error('Kalshi fetch error:', error);
            throw error;
        }
    }

    /**
     * Normalize Polymarket data to internal format
     */
    normalizePolymarketData(data) {
        // Map to internal format matching Rust data_normalizer.rs
        const outcomes = data.outcomes || [];
        const prices = this.extractPolymarketPrices(data);
        
        return {
            marketId: data.id || data.conditionId,
            source: MarketSource.POLYMARKET,
            externalId: data.id,
            title: data.question || data.title,
            description: data.description || '',
            outcomes: outcomes.map((outcome, index) => ({
                index,
                name: outcome.name || outcome,
                price: prices[index] || 0.5,
                probability: Math.round(prices[index] * 10000), // Convert to basis points
                volume: outcome.volume || 0
            })),
            prices: {
                bid: data.bestBid || prices[0] * 0.99,
                ask: data.bestAsk || prices[0] * 1.01,
                mid: prices[0] || 0.5,
                last: data.lastPrice || prices[0],
                change24h: data.priceChange24h || 0,
                high24h: data.high24h || prices[0] * 1.1,
                low24h: data.low24h || prices[0] * 0.9
            },
            volume: {
                total24h: data.volume24h || 0,
                buy24h: data.buyVolume24h || data.volume24h * 0.45,
                sell24h: data.sellVolume24h || data.volume24h * 0.55,
                trades24h: data.trades24h || 0,
                uniqueTraders: data.traders || 0
            },
            liquidity: data.liquidity || 0,
            status: this.mapPolymarketStatus(data.status),
            metadata: {
                category: this.extractCategory(data),
                tags: data.tags || [],
                resolutionTime: data.endDate,
                createTime: data.createdAt,
                updateTime: Date.now(),
                disputeInfo: data.disputed ? {
                    reason: data.disputeReason,
                    raisedAt: data.disputeTime,
                    raisedBy: null,
                    evidenceUrl: null
                } : null
            },
            timestamp: Date.now(),
            version: 1
        };
    }

    /**
     * Normalize Kalshi data to internal format
     */
    normalizeKalshiData(data) {
        const market = data.market || data;
        const outcomes = this.extractKalshiOutcomes(market);
        
        return {
            marketId: market.ticker,
            source: MarketSource.KALSHI,
            externalId: market.ticker,
            title: market.title,
            description: market.rules || '',
            outcomes: outcomes,
            prices: {
                bid: market.bid_price / 100 || 0.49,
                ask: market.ask_price / 100 || 0.51,
                mid: (market.bid_price + market.ask_price) / 200 || 0.5,
                last: market.last_price / 100 || 0.5,
                change24h: 0,
                high24h: market.high_price / 100 || 0.6,
                low24h: market.low_price / 100 || 0.4
            },
            volume: {
                total24h: market.volume_24h || 0,
                buy24h: market.volume_24h * 0.45 || 0,
                sell24h: market.volume_24h * 0.55 || 0,
                trades24h: market.trades_24h || 0,
                uniqueTraders: 0
            },
            liquidity: market.open_interest || 0,
            status: this.mapKalshiStatus(market.status),
            metadata: {
                category: market.category,
                tags: [market.category],
                resolutionTime: market.close_time,
                createTime: market.created_time,
                updateTime: Date.now(),
                disputeInfo: null
            },
            timestamp: Date.now(),
            version: 1
        };
    }

    /**
     * Extract Polymarket prices
     */
    extractPolymarketPrices(data) {
        if (data.outcomeDetails) {
            return data.outcomeDetails.map(o => o.price || 0.5);
        } else if (data.yesPrice !== undefined && data.noPrice !== undefined) {
            return [data.yesPrice, data.noPrice];
        } else if (data.prices) {
            return data.prices;
        }
        return [0.5, 0.5];
    }

    /**
     * Extract Kalshi outcomes
     */
    extractKalshiOutcomes(market) {
        if (market.is_binary) {
            return [
                {
                    index: 0,
                    name: 'Yes',
                    price: market.yes_price / 100 || 0.5,
                    probability: market.yes_price || 5000,
                    volume: market.volume_24h / 2 || 0
                },
                {
                    index: 1,
                    name: 'No',
                    price: market.no_price / 100 || 0.5,
                    probability: market.no_price || 5000,
                    volume: market.volume_24h / 2 || 0
                }
            ];
        }
        // Handle non-binary markets
        return [];
    }

    /**
     * Map Polymarket status
     */
    mapPolymarketStatus(status) {
        const statusMap = {
            'active': MarketStatus.ACTIVE,
            'closed': MarketStatus.CLOSED,
            'resolved': MarketStatus.RESOLVED,
            'disputed': MarketStatus.DISPUTED,
            'cancelled': MarketStatus.CANCELLED
        };
        return statusMap[status?.toLowerCase()] || MarketStatus.ACTIVE;
    }

    /**
     * Map Kalshi status
     */
    mapKalshiStatus(status) {
        const statusMap = {
            'open': MarketStatus.ACTIVE,
            'closed': MarketStatus.CLOSED,
            'settled': MarketStatus.RESOLVED,
            'halted': MarketStatus.PAUSED
        };
        return statusMap[status?.toLowerCase()] || MarketStatus.ACTIVE;
    }

    /**
     * Extract category from market data
     */
    extractCategory(data) {
        const title = (data.title || data.question || '').toLowerCase();
        
        if (title.includes('election') || title.includes('president')) {
            return 'Politics';
        } else if (title.includes('bitcoin') || title.includes('crypto')) {
            return 'Crypto';
        } else if (title.includes('sports') || title.includes('game')) {
            return 'Sports';
        } else if (title.includes('stock') || title.includes('market')) {
            return 'Finance';
        }
        
        return data.category || 'General';
    }

    /**
     * Search markets
     */
    async searchMarkets(query, source = null) {
        const results = [];
        
        if (!source || source === MarketSource.POLYMARKET) {
            const polymarkets = await this.searchPolymarket(query);
            results.push(...polymarkets);
        }
        
        if (!source || source === MarketSource.KALSHI) {
            const kalshiMarkets = await this.searchKalshi(query);
            results.push(...kalshiMarkets);
        }
        
        return results;
    }

    /**
     * Search Polymarket
     */
    async searchPolymarket(query) {
        try {
            const response = await fetch(`${POLYMARKET_API}/markets?search=${encodeURIComponent(query)}&limit=10`);
            if (!response.ok) return [];
            
            const data = await response.json();
            return data.map(market => this.normalizePolymarketData(market));
        } catch (error) {
            console.error('Polymarket search error:', error);
            return [];
        }
    }

    /**
     * Search Kalshi
     */
    async searchKalshi(query) {
        try {
            const response = await fetch(`${KALSHI_API}/markets?q=${encodeURIComponent(query)}&limit=10`);
            if (!response.ok) return [];
            
            const data = await response.json();
            return data.markets.map(market => this.normalizeKalshiData(market));
        } catch (error) {
            console.error('Kalshi search error:', error);
            return [];
        }
    }

    /**
     * Subscribe to market updates via WebSocket
     */
    subscribeToMarketUpdates(marketId, source, callback) {
        const key = `${source}_${marketId}`;
        
        if (!this.subscriptions.has(key)) {
            this.subscriptions.set(key, new Set());
        }
        
        this.subscriptions.get(key).add(callback);
        
        // Connect WebSocket if needed
        if (source === MarketSource.POLYMARKET) {
            this.connectPolymarketWS(marketId);
        }
        
        return () => {
            const subs = this.subscriptions.get(key);
            if (subs) {
                subs.delete(callback);
                if (subs.size === 0) {
                    this.subscriptions.delete(key);
                }
            }
        };
    }

    /**
     * Connect Polymarket WebSocket
     */
    connectPolymarketWS(marketId) {
        if (this.wsConnections.has('polymarket')) return;
        
        const ws = new WebSocket('wss://ws.polymarket.com');
        
        ws.onopen = () => {
            ws.send(JSON.stringify({
                type: 'subscribe',
                channel: 'market',
                market: marketId
            }));
        };
        
        ws.onmessage = (event) => {
            const data = JSON.parse(event.data);
            const key = `${MarketSource.POLYMARKET}_${data.market}`;
            const callbacks = this.subscriptions.get(key);
            
            if (callbacks) {
                const normalized = this.normalizePolymarketData(data);
                callbacks.forEach(cb => cb(normalized));
            }
        };
        
        ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
        
        this.wsConnections.set('polymarket', ws);
    }

    /**
     * Get from cache
     */
    getFromCache(key) {
        const cached = this.cache.get(key);
        if (cached && Date.now() - cached.timestamp < CACHE_DURATION) {
            return cached.data;
        }
        return null;
    }

    /**
     * Set cache
     */
    setCache(key, data) {
        this.cache.set(key, {
            data,
            timestamp: Date.now()
        });
    }

    /**
     * Clear cache
     */
    clearCache() {
        this.cache.clear();
    }

    /**
     * Get trending markets
     */
    async getTrendingMarkets() {
        const polymarkets = await fetch(`${POLYMARKET_API}/markets?sort=volume&limit=5`)
            .then(r => r.json())
            .then(data => data.map(m => this.normalizePolymarketData(m)))
            .catch(() => []);

        return polymarkets;
    }

    /**
     * Get markets by category
     */
    async getMarketsByCategory(category) {
        const results = [];
        
        // Polymarket
        const polyResponse = await fetch(`${POLYMARKET_API}/markets?tag=${category.toLowerCase()}&limit=10`)
            .then(r => r.json())
            .then(data => data.map(m => this.normalizePolymarketData(m)))
            .catch(() => []);
        
        results.push(...polyResponse);
        
        return results;
    }
}

// Export singleton instance
export const marketDataService = new MarketDataService();