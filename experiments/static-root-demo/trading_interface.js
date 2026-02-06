/**
 * Trading Interface Module
 * Handles trading logic, order management, and UI updates
 * Integrates with Solana blockchain for order execution
 */

import { solanaIntegration } from './solana_integration.js';
import { marketDataService } from './market_data.js';
import { verseSystem } from './verse_system.js';
import { quantumCalculator } from './quantum_mode.js';

// Order types
export const OrderType = {
    MARKET: 0,
    LIMIT: 1,
    STOP_LOSS: 2,
    TAKE_PROFIT: 3,
    TRAILING_STOP: 4
};

// Order status
export const OrderStatus = {
    PENDING: 0,
    OPEN: 1,
    PARTIALLY_FILLED: 2,
    FILLED: 3,
    CANCELLED: 4,
    FAILED: 5
};

/**
 * Trading interface manager
 */
export class TradingInterface {
    constructor() {
        this.orders = new Map();
        this.positions = new Map();
        this.selectedMarket = null;
        this.selectedVerses = [];
        this.baseLeverage = 1;
        this.quantumMode = false;
        this.listeners = new Map();
    }

    /**
     * Initialize trading interface
     */
    async initialize() {
        // Initialize dependencies
        await verseSystem.initialize();
        
        // Set up event listeners
        this.setupEventListeners();
        
        // Load user positions if wallet connected
        if (solanaIntegration.wallet) {
            await this.loadUserPositions();
        }
    }

    /**
     * Setup event listeners
     */
    setupEventListeners() {
        // Market selection
        document.addEventListener('market-selected', (e) => {
            this.selectMarket(e.detail);
        });

        // Verse selection
        document.addEventListener('verse-selected', (e) => {
            this.toggleVerse(e.detail.verseId);
        });

        // Leverage change
        document.addEventListener('leverage-changed', (e) => {
            this.setBaseLeverage(e.detail.leverage);
        });

        // Quantum mode toggle
        document.addEventListener('quantum-toggled', (e) => {
            this.toggleQuantumMode();
        });
    }

    /**
     * Select market
     */
    async selectMarket(marketData) {
        this.selectedMarket = marketData;
        
        // Get available verses for this market
        const verses = await verseSystem.getVersesForMarket(marketData);
        
        // Update UI
        this.emit('market-changed', {
            market: marketData,
            availableVerses: verses
        });

        // Subscribe to market updates
        this.subscribeToMarketUpdates(marketData);
    }

    /**
     * Toggle verse selection
     */
    toggleVerse(verseId) {
        const index = this.selectedVerses.indexOf(verseId);
        
        if (index === -1) {
            this.selectedVerses.push(verseId);
        } else {
            this.selectedVerses.splice(index, 1);
        }

        // Calculate total leverage
        const totalLeverage = this.calculateTotalLeverage();
        
        this.emit('verses-changed', {
            selectedVerses: this.selectedVerses,
            totalLeverage
        });
    }

    /**
     * Set base leverage
     */
    setBaseLeverage(leverage) {
        this.baseLeverage = Math.min(Math.max(leverage, 1), 100);
        
        const totalLeverage = this.calculateTotalLeverage();
        
        this.emit('leverage-updated', {
            baseLeverage: this.baseLeverage,
            totalLeverage
        });
    }

    /**
     * Toggle quantum mode
     */
    toggleQuantumMode() {
        this.quantumMode = !this.quantumMode;
        
        this.emit('quantum-mode-changed', {
            enabled: this.quantumMode
        });
    }

    /**
     * Calculate total leverage
     */
    calculateTotalLeverage() {
        return verseSystem.calculateTotalLeverage(
            this.baseLeverage,
            this.selectedVerses
        );
    }

    /**
     * Place order
     */
    async placeOrder(params) {
        const {
            outcome,
            amount,
            orderType = OrderType.MARKET,
            limitPrice = null,
            stopLoss = null,
            takeProfit = null
        } = params;

        if (!this.selectedMarket) {
            throw new Error('No market selected');
        }

        if (!solanaIntegration.wallet) {
            throw new Error('Wallet not connected');
        }

        // Validate order
        this.validateOrder(params);

        // Create order object
        const order = {
            id: `order_${Date.now()}`,
            marketId: this.selectedMarket.marketId,
            outcome,
            amount,
            orderType,
            limitPrice,
            stopLoss,
            takeProfit,
            leverage: this.calculateTotalLeverage(),
            verses: [...this.selectedVerses],
            quantumEnabled: this.quantumMode,
            status: OrderStatus.PENDING,
            created: Date.now()
        };

        try {
            // Show pending state
            this.orders.set(order.id, order);
            this.emit('order-pending', order);

            // Create quantum position if enabled
            if (this.quantumMode) {
                const quantumPosition = quantumCalculator.createQuantumPosition(
                    this.selectedMarket.marketId,
                    this.selectedMarket.outcomes,
                    amount,
                    order.leverage
                );
                order.quantumState = quantumPosition.state;
            }

            // Execute on blockchain
            const instruction = await solanaIntegration.placeBetInstruction({
                marketId: this.selectedMarket.marketId,
                outcome: this.selectedMarket.outcomes.findIndex(o => o.name === outcome),
                amount,
                leverage: order.leverage,
                isQuantum: this.quantumMode
            });

            const result = await solanaIntegration.executeTransaction([instruction]);
            
            // Update order status
            order.status = OrderStatus.OPEN;
            order.signature = result.signature;
            
            this.emit('order-placed', order);

            // Create position
            this.createPosition(order);

            return order;
        } catch (error) {
            // Update order status
            order.status = OrderStatus.FAILED;
            order.error = error.message;
            
            this.emit('order-failed', {
                order,
                error: error.message
            });

            throw error;
        }
    }

    /**
     * Validate order parameters
     */
    validateOrder(params) {
        const { amount, outcome, limitPrice } = params;

        // Check minimum amount
        if (amount < 0.1) {
            throw new Error('Minimum order amount is 0.1 SOL');
        }

        // Check outcome exists
        const outcomeExists = this.selectedMarket.outcomes.some(o => o.name === outcome);
        if (!outcomeExists) {
            throw new Error('Invalid outcome selected');
        }

        // Check limit price
        if (params.orderType === OrderType.LIMIT && (!limitPrice || limitPrice <= 0 || limitPrice >= 1)) {
            throw new Error('Invalid limit price');
        }

        // Check balance
        // This would check actual wallet balance
    }

    /**
     * Create position from order
     */
    createPosition(order) {
        const position = {
            id: `pos_${Date.now()}`,
            orderId: order.id,
            marketId: order.marketId,
            market: this.selectedMarket,
            outcome: order.outcome,
            amount: order.amount,
            leverage: order.leverage,
            verses: order.verses,
            quantumEnabled: order.quantumEnabled,
            quantumState: order.quantumState,
            entryPrice: this.selectedMarket.outcomes.find(o => o.name === order.outcome).price,
            currentPrice: this.selectedMarket.outcomes.find(o => o.name === order.outcome).price,
            pnl: 0,
            pnlPercent: 0,
            status: 'open',
            created: Date.now()
        };

        this.positions.set(position.id, position);
        this.emit('position-created', position);

        // Start monitoring position
        this.monitorPosition(position);
    }

    /**
     * Monitor position for updates
     */
    monitorPosition(position) {
        const interval = setInterval(() => {
            if (position.status !== 'open') {
                clearInterval(interval);
                return;
            }

            // Update current price
            const outcome = this.selectedMarket.outcomes.find(o => o.name === position.outcome);
            if (outcome) {
                position.currentPrice = outcome.price;
                
                // Calculate PnL
                const priceChange = position.currentPrice - position.entryPrice;
                const direction = position.outcome === 'Yes' ? 1 : -1;
                position.pnl = position.amount * position.leverage * priceChange * direction;
                position.pnlPercent = (priceChange / position.entryPrice) * 100 * direction;

                this.emit('position-updated', position);

                // Check stop loss / take profit
                this.checkPositionLimits(position);
            }
        }, 1000); // Update every second
    }

    /**
     * Check position limits (stop loss, take profit)
     */
    checkPositionLimits(position) {
        const order = Array.from(this.orders.values()).find(o => o.id === position.orderId);
        if (!order) return;

        // Check stop loss
        if (order.stopLoss && position.pnl <= -order.stopLoss) {
            this.closePosition(position.id, 'stop_loss');
        }

        // Check take profit
        if (order.takeProfit && position.pnl >= order.takeProfit) {
            this.closePosition(position.id, 'take_profit');
        }
    }

    /**
     * Close position
     */
    async closePosition(positionId, reason = 'manual') {
        const position = this.positions.get(positionId);
        if (!position || position.status !== 'open') return;

        try {
            position.status = 'closing';
            this.emit('position-closing', position);

            // Execute close on blockchain
            // This would create a closing transaction

            position.status = 'closed';
            position.closedAt = Date.now();
            position.closeReason = reason;

            // Resolve quantum state if enabled
            if (position.quantumEnabled) {
                const resolution = quantumCalculator.resolvePosition(
                    position.marketId,
                    position.outcome
                );
                position.quantumResolution = resolution;
            }

            this.emit('position-closed', position);
        } catch (error) {
            position.status = 'open';
            this.emit('position-close-failed', {
                position,
                error: error.message
            });
        }
    }

    /**
     * Cancel order
     */
    async cancelOrder(orderId) {
        const order = this.orders.get(orderId);
        if (!order || order.status !== OrderStatus.OPEN) return;

        try {
            order.status = OrderStatus.CANCELLED;
            this.emit('order-cancelled', order);
        } catch (error) {
            this.emit('order-cancel-failed', {
                order,
                error: error.message
            });
        }
    }

    /**
     * Load user positions from blockchain
     */
    async loadUserPositions() {
        try {
            const positions = await solanaIntegration.getUserPositions();
            
            for (const posData of positions) {
                // Convert blockchain data to position object
                const position = this.parsePositionData(posData);
                this.positions.set(position.id, position);
            }

            this.emit('positions-loaded', Array.from(this.positions.values()));
        } catch (error) {
            console.error('Failed to load positions:', error);
        }
    }

    /**
     * Parse position data from blockchain
     */
    parsePositionData(data) {
        // Implementation depends on exact data structure
        return {
            id: data.marketId,
            marketId: data.marketId,
            outcome: data.outcome,
            amount: Number(data.amount) / 1e9, // Convert from lamports
            leverage: data.leverage,
            status: 'open'
        };
    }

    /**
     * Subscribe to market updates
     */
    subscribeToMarketUpdates(marketData) {
        const unsubscribe = marketDataService.subscribeToMarketUpdates(
            marketData.marketId,
            marketData.source,
            (updatedMarket) => {
                this.selectedMarket = updatedMarket;
                this.emit('market-updated', updatedMarket);
            }
        );

        // Store unsubscribe function
        this.listeners.set('market-updates', unsubscribe);
    }

    /**
     * Get portfolio summary
     */
    getPortfolioSummary() {
        const positions = Array.from(this.positions.values());
        const openPositions = positions.filter(p => p.status === 'open');
        
        const summary = {
            totalPositions: positions.length,
            openPositions: openPositions.length,
            totalValue: 0,
            totalPnl: 0,
            totalPnlPercent: 0,
            quantumPositions: 0,
            byMarket: new Map(),
            byOutcome: new Map()
        };

        for (const position of openPositions) {
            summary.totalValue += position.amount * position.leverage;
            summary.totalPnl += position.pnl || 0;
            
            if (position.quantumEnabled) {
                summary.quantumPositions++;
            }

            // Group by market
            const marketKey = position.market?.title || position.marketId;
            if (!summary.byMarket.has(marketKey)) {
                summary.byMarket.set(marketKey, {
                    count: 0,
                    value: 0,
                    pnl: 0
                });
            }
            const marketStats = summary.byMarket.get(marketKey);
            marketStats.count++;
            marketStats.value += position.amount * position.leverage;
            marketStats.pnl += position.pnl || 0;

            // Group by outcome
            if (!summary.byOutcome.has(position.outcome)) {
                summary.byOutcome.set(position.outcome, {
                    count: 0,
                    value: 0,
                    pnl: 0
                });
            }
            const outcomeStats = summary.byOutcome.get(position.outcome);
            outcomeStats.count++;
            outcomeStats.value += position.amount * position.leverage;
            outcomeStats.pnl += position.pnl || 0;
        }

        if (summary.totalValue > 0) {
            summary.totalPnlPercent = (summary.totalPnl / summary.totalValue) * 100;
        }

        return summary;
    }

    /**
     * Emit event to listeners
     */
    emit(event, data) {
        const customEvent = new CustomEvent(`trading-${event}`, { detail: data });
        document.dispatchEvent(customEvent);
    }

    /**
     * Clean up
     */
    cleanup() {
        // Unsubscribe from all listeners
        for (const unsubscribe of this.listeners.values()) {
            if (typeof unsubscribe === 'function') {
                unsubscribe();
            }
        }
        this.listeners.clear();
    }
}

// Export singleton instance
export const tradingInterface = new TradingInterface();