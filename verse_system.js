/**
 * Verse System Module
 * Implements hierarchical verse structure and leverage calculations
 * Based on the Native Solana verse implementation
 */

import { solanaIntegration } from './solana_integration.js';
import { marketDataService, MarketSource } from './market_data.js';

// Verse types matching Rust enum
export const VerseType = {
    MAIN: 0,
    QUANTUM: 1,
    DISTRIBUTION: 2,
    ROOT: 3,
    CATEGORY: 4,
    SUBCATEGORY: 5,
    MARKET: 6
};

// Maximum values
const MAX_LEVERAGE = 100;
const MAX_VERSE_DEPTH = 4;
const MAX_CHILDREN = 10;

/**
 * Verse node in the hierarchy
 */
class VerseNode {
    constructor(data) {
        this.id = data.id;
        this.name = data.name;
        this.type = data.type;
        this.level = data.level;
        this.parentId = data.parentId;
        this.children = [];
        this.markets = data.markets || [];
        this.multiplier = data.multiplier || 1;
        this.description = data.description;
        this.source = data.source;
        this.active = data.active !== false;
        this.quantumEnabled = data.quantumEnabled || false;
    }

    addChild(child) {
        if (this.children.length >= MAX_CHILDREN) {
            throw new Error('Maximum children exceeded');
        }
        this.children.push(child);
    }

    getTotalMultiplier() {
        let total = this.multiplier;
        let parent = this.parent;
        
        while (parent) {
            total *= parent.multiplier;
            parent = parent.parent;
        }
        
        return Math.min(total, MAX_LEVERAGE);
    }
}

/**
 * Verse system manager
 */
export class VerseSystem {
    constructor() {
        this.verses = new Map();
        this.rootVerses = [];
        this.marketToVerse = new Map();
        this.initialized = false;
    }

    /**
     * Initialize verse system
     */
    async initialize() {
        if (this.initialized) return;

        // Create default verse hierarchy
        await this.createDefaultVerses();
        
        // Load verses from blockchain if connected
        if (solanaIntegration.wallet) {
            await this.loadVersesFromChain();
        }

        this.initialized = true;
    }

    /**
     * Create default verse hierarchy
     */
    async createDefaultVerses() {
        // Main root verses by category
        const categories = [
            {
                id: 'politics-root',
                name: 'Politics & Elections',
                type: VerseType.ROOT,
                children: [
                    {
                        id: 'us-politics',
                        name: 'US Politics',
                        type: VerseType.CATEGORY,
                        multiplier: 1.5,
                        children: [
                            {
                                id: 'presidential-2024',
                                name: '2024 Presidential Election',
                                type: VerseType.SUBCATEGORY,
                                multiplier: 2,
                                markets: ['polymarket-presidential']
                            },
                            {
                                id: 'congress-2024',
                                name: '2024 Congressional',
                                type: VerseType.SUBCATEGORY,
                                multiplier: 1.5
                            }
                        ]
                    },
                    {
                        id: 'world-politics',
                        name: 'World Politics',
                        type: VerseType.CATEGORY,
                        multiplier: 1.3
                    }
                ]
            },
            {
                id: 'crypto-root',
                name: 'Cryptocurrency',
                type: VerseType.ROOT,
                children: [
                    {
                        id: 'bitcoin-verse',
                        name: 'Bitcoin Markets',
                        type: VerseType.CATEGORY,
                        multiplier: 2,
                        children: [
                            {
                                id: 'btc-price',
                                name: 'BTC Price Predictions',
                                type: VerseType.SUBCATEGORY,
                                multiplier: 1.5
                            },
                            {
                                id: 'btc-adoption',
                                name: 'BTC Adoption',
                                type: VerseType.SUBCATEGORY,
                                multiplier: 1.2
                            }
                        ]
                    },
                    {
                        id: 'ethereum-verse',
                        name: 'Ethereum Markets',
                        type: VerseType.CATEGORY,
                        multiplier: 1.8
                    },
                    {
                        id: 'defi-verse',
                        name: 'DeFi & Web3',
                        type: VerseType.CATEGORY,
                        multiplier: 2.5
                    }
                ]
            },
            {
                id: 'tech-root',
                name: 'Technology',
                type: VerseType.ROOT,
                children: [
                    {
                        id: 'ai-verse',
                        name: 'AI & AGI',
                        type: VerseType.CATEGORY,
                        multiplier: 3,
                        children: [
                            {
                                id: 'agi-timeline',
                                name: 'AGI Timeline',
                                type: VerseType.SUBCATEGORY,
                                multiplier: 2
                            },
                            {
                                id: 'ai-companies',
                                name: 'AI Companies',
                                type: VerseType.SUBCATEGORY,
                                multiplier: 1.5
                            }
                        ]
                    }
                ]
            },
            {
                id: 'sports-root',
                name: 'Sports',
                type: VerseType.ROOT,
                children: [
                    {
                        id: 'nfl-verse',
                        name: 'NFL',
                        type: VerseType.CATEGORY,
                        multiplier: 1.5
                    },
                    {
                        id: 'nba-verse',
                        name: 'NBA',
                        type: VerseType.CATEGORY,
                        multiplier: 1.5
                    }
                ]
            }
        ];

        // Build verse tree
        for (const categoryData of categories) {
            const rootVerse = await this.createVerse(categoryData);
            this.rootVerses.push(rootVerse);
        }
    }

    /**
     * Create verse recursively
     */
    async createVerse(data, parent = null) {
        const verse = new VerseNode({
            ...data,
            level: parent ? parent.level + 1 : 0,
            parentId: parent?.id
        });

        verse.parent = parent;
        this.verses.set(verse.id, verse);

        // Add to parent
        if (parent) {
            parent.addChild(verse);
        }

        // Create children
        if (data.children) {
            for (const childData of data.children) {
                await this.createVerse(childData, verse);
            }
        }

        // Map markets to verse
        if (verse.markets) {
            for (const marketId of verse.markets) {
                this.marketToVerse.set(marketId, verse.id);
            }
        }

        return verse;
    }

    /**
     * Load verses from blockchain
     */
    async loadVersesFromChain() {
        try {
            // This would fetch verse accounts from Solana
            console.log('Loading verses from chain...');
            // Implementation depends on your program's account structure
        } catch (error) {
            console.error('Failed to load verses from chain:', error);
        }
    }

    /**
     * Get verse by ID
     */
    getVerse(verseId) {
        return this.verses.get(verseId);
    }

    /**
     * Get all root verses
     */
    getRootVerses() {
        return this.rootVerses;
    }

    /**
     * Get verses for market
     */
    async getVersesForMarket(marketData) {
        const verses = [];
        
        // Find verses based on market category and content
        const category = marketData.metadata.category.toLowerCase();
        const title = marketData.title.toLowerCase();
        
        // Search through verse tree
        for (const root of this.rootVerses) {
            const matchingVerses = this.findMatchingVerses(root, marketData);
            verses.push(...matchingVerses);
        }

        // Generate dynamic verses based on market content
        const dynamicVerses = await this.generateDynamicVerses(marketData);
        verses.push(...dynamicVerses);

        return verses;
    }

    /**
     * Find matching verses recursively
     */
    findMatchingVerses(verse, marketData) {
        const matches = [];
        const title = marketData.title.toLowerCase();
        const category = marketData.metadata.category.toLowerCase();

        // Check if verse matches
        if (this.verseMatchesMarket(verse, title, category)) {
            matches.push(verse);
        }

        // Check children
        for (const child of verse.children) {
            matches.push(...this.findMatchingVerses(child, marketData));
        }

        return matches;
    }

    /**
     * Check if verse matches market
     */
    verseMatchesMarket(verse, title, category) {
        const verseName = verse.name.toLowerCase();
        
        // Category match
        if (verse.type === VerseType.ROOT || verse.type === VerseType.CATEGORY) {
            if (verseName.includes(category) || category.includes(verseName)) {
                return true;
            }
        }

        // Keyword matching
        const keywords = verseName.split(/\s+/);
        for (const keyword of keywords) {
            if (keyword.length > 3 && title.includes(keyword)) {
                return true;
            }
        }

        // Specific market mapping
        if (verse.markets && verse.markets.some(m => title.includes(m))) {
            return true;
        }

        return false;
    }

    /**
     * Generate dynamic verses based on market
     */
    async generateDynamicVerses(marketData) {
        const verses = [];
        const title = marketData.title;
        
        // Time-based verses
        if (title.includes('2024') || title.includes('2025')) {
            verses.push({
                id: 'time-2024-2025',
                name: '2024-2025 Events',
                type: VerseType.SUBCATEGORY,
                multiplier: 1.3,
                level: 2
            });
        }

        // Entity-based verses
        const entities = this.extractEntities(title);
        for (const entity of entities) {
            verses.push({
                id: `entity-${entity.toLowerCase().replace(/\s+/g, '-')}`,
                name: `${entity} Markets`,
                type: VerseType.SUBCATEGORY,
                multiplier: 1.5,
                level: 2
            });
        }

        // Outcome-based verses
        if (marketData.outcomes.length === 2) {
            verses.push({
                id: 'binary-high-confidence',
                name: 'High Confidence Binary',
                type: VerseType.SUBCATEGORY,
                multiplier: 1.2,
                level: 2,
                condition: () => {
                    const probs = marketData.outcomes.map(o => o.probability);
                    return Math.max(...probs) > 7000; // 70%+ probability
                }
            });
        }

        return verses.filter(v => !v.condition || v.condition());
    }

    /**
     * Extract entities from text
     */
    extractEntities(text) {
        const entities = [];
        
        // Common patterns
        const patterns = [
            /(?:will\s+)([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)/g,
            /([A-Z][a-zA-Z]+(?:\s+[A-Z][a-zA-Z]+)*)\s+(?:win|reach|achieve)/g,
        ];

        for (const pattern of patterns) {
            let match;
            while ((match = pattern.exec(text)) !== null) {
                if (match[1] && match[1].length > 3) {
                    entities.push(match[1]);
                }
            }
        }

        return [...new Set(entities)];
    }

    /**
     * Calculate total leverage for position
     */
    calculateTotalLeverage(baseLeverage, verseIds) {
        let totalMultiplier = 1;
        
        for (const verseId of verseIds) {
            const verse = this.getVerse(verseId);
            if (verse) {
                totalMultiplier *= verse.multiplier;
            }
        }

        const totalLeverage = baseLeverage * totalMultiplier;
        return Math.min(totalLeverage, MAX_LEVERAGE);
    }

    /**
     * Get verse path (from root to verse)
     */
    getVersePath(verseId) {
        const path = [];
        let verse = this.getVerse(verseId);
        
        while (verse) {
            path.unshift(verse);
            verse = verse.parent;
        }

        return path;
    }

    /**
     * Search verses by name
     */
    searchVerses(query) {
        const results = [];
        const searchTerm = query.toLowerCase();
        
        for (const verse of this.verses.values()) {
            if (verse.name.toLowerCase().includes(searchTerm)) {
                results.push({
                    verse,
                    path: this.getVersePath(verse.id),
                    relevance: this.calculateRelevance(verse.name, searchTerm)
                });
            }
        }

        return results.sort((a, b) => b.relevance - a.relevance);
    }

    /**
     * Calculate search relevance
     */
    calculateRelevance(name, searchTerm) {
        const nameLower = name.toLowerCase();
        
        // Exact match
        if (nameLower === searchTerm) return 100;
        
        // Starts with
        if (nameLower.startsWith(searchTerm)) return 80;
        
        // Word match
        const words = nameLower.split(/\s+/);
        if (words.some(w => w === searchTerm)) return 60;
        
        // Contains
        return 40;
    }

    /**
     * Get verse statistics
     */
    getVerseStats(verseId) {
        const verse = this.getVerse(verseId);
        if (!verse) return null;

        const stats = {
            totalMarkets: 0,
            totalVolume: 0,
            avgMultiplier: verse.multiplier,
            depth: 0,
            activeChildren: 0
        };

        // Calculate stats recursively
        this.calculateVerseStats(verse, stats);

        return stats;
    }

    /**
     * Calculate verse statistics recursively
     */
    calculateVerseStats(verse, stats, depth = 0) {
        stats.depth = Math.max(stats.depth, depth);
        stats.totalMarkets += verse.markets.length;
        
        if (verse.active) {
            stats.activeChildren++;
        }

        for (const child of verse.children) {
            this.calculateVerseStats(child, stats, depth + 1);
        }
    }

    /**
     * Create custom verse
     */
    async createCustomVerse(params) {
        const {
            name,
            parentId,
            multiplier = 1,
            description,
            markets = []
        } = params;

        const parent = parentId ? this.getVerse(parentId) : null;
        
        if (parent && parent.level >= MAX_VERSE_DEPTH - 1) {
            throw new Error('Maximum verse depth exceeded');
        }

        const verseData = {
            id: `custom-${Date.now()}`,
            name,
            type: VerseType.SUBCATEGORY,
            multiplier: Math.min(multiplier, 5), // Limit custom multipliers
            description,
            markets
        };

        return await this.createVerse(verseData, parent);
    }

    /**
     * Export verse tree as JSON
     */
    exportVerseTree() {
        const exportVerse = (verse) => ({
            id: verse.id,
            name: verse.name,
            type: verse.type,
            level: verse.level,
            multiplier: verse.multiplier,
            markets: verse.markets,
            children: verse.children.map(exportVerse)
        });

        return this.rootVerses.map(exportVerse);
    }
}

// Export singleton instance
export const verseSystem = new VerseSystem();