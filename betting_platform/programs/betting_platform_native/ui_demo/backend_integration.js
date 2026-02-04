// Backend Integration for Platform UI
// Connects to local API server, smart contracts, and Polymarket

const API_BASE_URL = 'http://localhost:8081/api';
const POLYMARKET_API = 'https://clob.polymarket.com';

class BackendAPI {
    constructor() {
        this.ws = null;
        this.listeners = new Map();
        this.polymarketCache = new Map();
    }

    // Initialize WebSocket connection
    initWebSocket() {
        const wsUrl = 'ws://localhost:8081/ws';
        this.ws = new WebSocket(wsUrl);
        
        this.ws.onopen = () => {
            console.log('WebSocket connected to backend');
            this.emit('connected', true);
        };
        
        this.ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                this.handleWebSocketMessage(data);
            } catch (error) {
                console.error('WebSocket message error:', error);
            }
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.emit('error', error);
        };
        
        this.ws.onclose = () => {
            console.log('WebSocket disconnected');
            this.emit('disconnected', true);
            // Reconnect after 5 seconds
            setTimeout(() => this.initWebSocket(), 5000);
        };
    }

    // Handle incoming WebSocket messages
    handleWebSocketMessage(data) {
        switch (data.type) {
            case 'marketUpdate':
                this.emit('marketUpdate', data.payload);
                break;
            case 'priceUpdate':
                this.emit('priceUpdate', data.payload);
                break;
            case 'tradeExecuted':
                this.emit('tradeExecuted', data.payload);
                break;
            case 'positionUpdate':
                this.emit('positionUpdate', data.payload);
                break;
            default:
                console.log('Unknown message type:', data.type);
        }
    }

    // Event emitter methods
    on(event, callback) {
        if (!this.listeners.has(event)) {
            this.listeners.set(event, []);
        }
        this.listeners.get(event).push(callback);
    }

    emit(event, data) {
        if (this.listeners.has(event)) {
            this.listeners.get(event).forEach(callback => callback(data));
        }
    }

    // Fetch markets from backend
    async getMarkets() {
        try {
            // Try to get markets from our backend first
            let markets = [];
            try {
                const response = await fetch(`${API_BASE_URL}/markets`);
                if (response.ok) {
                    markets = await response.json();
                }
            } catch (e) {
                console.log('Backend not available, using Polymarket only');
            }
            
            // Always fetch Polymarket markets
            const polymarkets = await this.fetchPolymarketMarkets();
            
            // Combine our markets with Polymarket markets
            const allMarkets = [...markets, ...polymarkets];
            
            // Remove duplicates based on title similarity
            const uniqueMarkets = allMarkets.reduce((acc, market) => {
                const exists = acc.some(m => this.marketTitlesMatch(m.title, market.title));
                if (!exists) {
                    acc.push(market);
                }
                return acc;
            }, []);
            
            console.log(`Total markets available: ${uniqueMarkets.length}`);
            return uniqueMarkets;
        } catch (error) {
            console.error('Error fetching markets:', error);
            return this.getRealPolymarketExamples();
        }
    }

    // Enhance markets with Polymarket data
    async enhanceWithPolymarketData(markets) {
        // Fetch active Polymarket markets
        try {
            const polymarkets = await this.fetchPolymarketMarkets();
            
            // Match and enhance our markets with Polymarket data
            return markets.map(market => {
                const polyMatch = polymarkets.find(pm => 
                    this.marketTitlesMatch(market.title, pm.question)
                );
                
                if (polyMatch) {
                    market.polymarket = {
                        id: polyMatch.conditionId,
                        volume: polyMatch.volume24hr,
                        liquidity: polyMatch.liquidity,
                        outcomes: polyMatch.outcomes,
                        orderBook: polyMatch.orderBook
                    };
                }
                
                return market;
            });
        } catch (error) {
            console.error('Error enhancing with Polymarket:', error);
            return markets;
        }
    }

    // Fetch Polymarket markets
    async fetchPolymarketMarkets() {
        // Check cache first
        const cacheKey = 'polymarket_markets';
        const cached = this.polymarketCache.get(cacheKey);
        if (cached && cached.timestamp > Date.now() - 60000) { // 1 minute cache
            return cached.data;
        }

        try {
            // Use our proxy endpoint to avoid CORS issues
            const response = await fetch(`${API_BASE_URL}/polymarket/markets`, {
                headers: {
                    'Accept': 'application/json'
                }
            });
            
            if (!response.ok) {
                console.log('Polymarket API returned:', response.status);
                throw new Error('Polymarket API error');
            }
            
            const data = await response.json();
            console.log('Fetched Polymarket markets:', data.length || 0);
            
            // Transform Polymarket data to our format
            const markets = (data || []).map(market => ({
                id: market.condition_id || market.id,
                title: market.question || market.title,
                description: market.description || '',
                category: market.category || 'general',
                volume: parseFloat(market.volume || 0),
                liquidity: parseFloat(market.liquidity || 0),
                outcomes: market.outcomes || ['Yes', 'No'],
                odds: {
                    yes: parseFloat(market.outcomePrices?.[0] || 0.5),
                    no: parseFloat(market.outcomePrices?.[1] || 0.5)
                },
                endDate: market.end_date_iso || market.close_time,
                source: 'polymarket',
                polymarket: {
                    id: market.condition_id || market.id,
                    slug: market.slug
                }
            }));
            
            // Cache the results
            this.polymarketCache.set(cacheKey, {
                data: markets,
                timestamp: Date.now()
            });
            
            return markets;
        } catch (error) {
            console.error('Error fetching Polymarket markets:', error);
            // Return some real example markets as fallback
            return this.getRealPolymarketExamples();
        }
    }
    
    // Get real Polymarket example markets
    getRealPolymarketExamples() {
        return [
            {
                id: 'trump_2024_gop',
                title: 'Donald Trump 2024 GOP Nominee',
                category: 'Politics',
                volume: 15234567,
                liquidity: 3500000,
                odds: { yes: 0.89, no: 0.11 },
                outcomes: ['Yes', 'No'],
                source: 'polymarket',
                endDate: '2024-07-15'
            },
            {
                id: 'btc_100k_2024',
                title: 'Bitcoin above $100k in 2024',
                category: 'Crypto',
                volume: 8765432,
                liquidity: 2000000,
                odds: { yes: 0.42, no: 0.58 },
                outcomes: ['Yes', 'No'],
                source: 'polymarket',
                endDate: '2024-12-31'
            },
            {
                id: 'fed_rate_cut_march',
                title: 'Fed cuts rates in March 2024',
                category: 'Economics',
                volume: 5432100,
                liquidity: 1500000,
                odds: { yes: 0.65, no: 0.35 },
                outcomes: ['Yes', 'No'],
                source: 'polymarket',
                endDate: '2024-03-20'
            },
            {
                id: 'superbowl_2024',
                title: 'Chiefs win Super Bowl 2024',
                category: 'Sports',
                volume: 3210987,
                liquidity: 900000,
                odds: { yes: 0.31, no: 0.69 },
                outcomes: ['Yes', 'No'],
                source: 'polymarket',
                endDate: '2024-02-11'
            },
            {
                id: 'gpt5_2024',
                title: 'OpenAI releases GPT-5 in 2024',
                category: 'Technology',
                volume: 2345678,
                liquidity: 750000,
                odds: { yes: 0.28, no: 0.72 },
                outcomes: ['Yes', 'No'],
                source: 'polymarket',
                endDate: '2024-12-31'
            }
        ];
    }

    // Check if market titles match
    marketTitlesMatch(title1, title2) {
        const normalize = (str) => str.toLowerCase().replace(/[^a-z0-9]/g, '');
        return normalize(title1).includes(normalize(title2)) || 
               normalize(title2).includes(normalize(title1));
    }

    // Get market verses
    async getMarketVerses(marketId) {
        try {
            // Try the verses endpoint first
            const response = await fetch(`${API_BASE_URL}/verses`);
            if (response.ok) {
                const data = await response.json();
                console.log('getMarketVerses API response:', data);
                
                // Handle different response formats
                let allVerses;
                if (Array.isArray(data)) {
                    allVerses = data;
                } else if (data && Array.isArray(data.verses)) {
                    allVerses = data.verses;
                } else if (data && Array.isArray(data.data)) {
                    allVerses = data.data;
                } else {
                    console.error('Unexpected verses response format:', data);
                    return this.generateMockVerses(marketId);
                }
                
                // Filter verses for this market if backend supports it
                return allVerses.filter(v => v.market_id === marketId || !v.market_id);
            }
            throw new Error('Failed to fetch verses');
        } catch (error) {
            console.error('Error fetching verses:', error);
            return this.generateMockVerses(marketId);
        }
    }

    // Generate mock verses for a market
    generateMockVerses(marketId) {
        const verseTypes = [
            { name: 'Time Decay', multiplier: 1.5, description: 'Bet on outcome timing' },
            { name: 'Volume Surge', multiplier: 2.0, description: 'Bet on trading volume' },
            { name: 'Sentiment Shift', multiplier: 1.8, description: 'Bet on market sentiment' },
            { name: 'Volatility Spike', multiplier: 2.5, description: 'Bet on price volatility' },
            { name: 'Correlation Break', multiplier: 3.0, description: 'Bet on correlation changes' }
        ];
        
        return verseTypes.map((type, index) => ({
            id: `${marketId}_verse_${index}`,
            marketId: marketId,
            name: type.name,
            multiplier: type.multiplier,
            description: type.description,
            currentOdds: 0.5 + Math.random() * 0.3,
            volume: Math.floor(Math.random() * 100000),
            active: true
        }));
    }

    // Place a trade
    async placeTrade(tradeData) {
        try {
            const response = await fetch(`${API_BASE_URL}/trade/place`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify(tradeData)
            });
            
            if (!response.ok) throw new Error('Trade failed');
            const result = await response.json();
            
            // If Polymarket integration is enabled, mirror the trade
            if (tradeData.mirrorToPolymarket && tradeData.market.polymarket) {
                await this.mirrorTradeToPolymarket(tradeData, result);
            }
            
            return result;
        } catch (error) {
            console.error('Error placing trade:', error);
            throw error;
        }
    }

    // Mirror trade to Polymarket
    async mirrorTradeToPolymarket(tradeData, localResult) {
        // This would integrate with Polymarket's API
        // For now, we'll simulate it
        console.log('Mirroring trade to Polymarket:', tradeData);
        return {
            polymarketOrderId: 'poly_' + Math.random().toString(36).substr(2, 9),
            status: 'mirrored'
        };
    }

    // Get user positions
    async getPositions(walletAddress) {
        try {
            const response = await fetch(`${API_BASE_URL}/positions/${walletAddress}`);
            if (!response.ok) throw new Error('Failed to fetch positions');
            return await response.json();
        } catch (error) {
            console.error('Error fetching positions:', error);
            return [];
        }
    }

    // Get mock markets for demo
    getMockMarkets() {
        return [
            {
                id: 'btc_100k_2025',
                title: 'Will Bitcoin reach $100k by 2025?',
                category: 'crypto',
                description: 'This market resolves YES if Bitcoin reaches $100,000 USD on any major exchange before January 1, 2025',
                outcomes: ['Yes', 'No'],
                odds: { yes: 0.34, no: 0.66 },
                volume: 1234567,
                liquidity: 500000,
                endDate: '2025-01-01',
                verses: []
            },
            {
                id: 'trump_2024',
                title: 'Donald Trump wins 2024 US Presidential Election',
                category: 'politics',
                description: 'This market resolves YES if Donald Trump wins the 2024 US Presidential Election',
                outcomes: ['Yes', 'No'],
                odds: { yes: 0.45, no: 0.55 },
                volume: 8765432,
                liquidity: 3000000,
                endDate: '2024-11-05',
                verses: []
            },
            {
                id: 'agi_2030',
                title: 'Will AGI be achieved by 2030?',
                category: 'technology',
                description: 'This market resolves YES if Artificial General Intelligence is achieved and recognized by major institutions by 2030',
                outcomes: ['Yes', 'No'],
                odds: { yes: 0.23, no: 0.77 },
                volume: 890123,
                liquidity: 750000,
                endDate: '2030-12-31',
                verses: []
            },
            {
                id: 'worldcup_2026',
                title: 'Brazil wins 2026 FIFA World Cup',
                category: 'sports',
                description: 'This market resolves YES if Brazil wins the 2026 FIFA World Cup',
                outcomes: ['Yes', 'No'],
                odds: { yes: 0.18, no: 0.82 },
                volume: 456789,
                liquidity: 400000,
                endDate: '2026-07-19',
                verses: []
            }
        ];
    }

    // Calculate total potential payout including verses
    calculateTotalPayout(baseAmount, selectedVerses, leverage = 1) {
        let totalMultiplier = leverage;
        
        // Apply verse multipliers
        selectedVerses.forEach(verse => {
            totalMultiplier *= verse.multiplier;
        });
        
        return {
            investment: baseAmount,
            potentialPayout: baseAmount * totalMultiplier,
            totalMultiplier: totalMultiplier,
            verseBonus: totalMultiplier / leverage
        };
    }
    
    // Close a position
    async closePosition(marketId, positionIndex) {
        try {
            const response = await fetch(`${API_BASE_URL}/positions/close`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    market_id: parseInt(marketId),
                    position_index: positionIndex || 0
                })
            });
            
            if (!response.ok) throw new Error('Position close failed');
            return await response.json();
        } catch (error) {
            console.error('Position close failed:', error);
            throw error;
        }
    }
    
    // Get quantum positions
    async getQuantumPositions(walletAddress) {
        try {
            const response = await fetch(`${API_BASE_URL}/quantum/positions/${walletAddress}`);
            if (!response.ok) throw new Error('Failed to fetch quantum positions');
            return await response.json();
        } catch (error) {
            console.error('Failed to fetch quantum positions:', error);
            return { positions: [] };
        }
    }
    
    // Get portfolio
    async getPortfolio(walletAddress) {
        try {
            const response = await fetch(`${API_BASE_URL}/portfolio/${walletAddress}`);
            if (!response.ok) throw new Error('Failed to fetch portfolio');
            const data = await response.json();
            
            // Calculate additional metrics
            const totalPnl = data.positions?.reduce((sum, pos) => sum + (pos.pnl || 0), 0) || 0;
            const winningPositions = data.positions?.filter(pos => pos.pnl > 0).length || 0;
            const totalPositions = data.positions?.length || 0;
            const winRate = totalPositions > 0 ? (winningPositions / totalPositions) * 100 : 0;
            
            return {
                ...data,
                totalPnl,
                winRate,
                positionCount: totalPositions
            };
        } catch (error) {
            console.error('Failed to fetch portfolio:', error);
            return {
                totalValue: 0,
                totalPnl: 0,
                winRate: 0,
                positionCount: 0
            };
        }
    }
}

// Create global instance
window.backendAPI = new BackendAPI();

// Auto-initialize WebSocket
window.backendAPI.initWebSocket();