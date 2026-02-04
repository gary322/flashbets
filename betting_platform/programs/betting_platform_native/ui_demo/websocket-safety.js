/**
 * Type-safe WebSocket wrapper for betting platform
 * Ensures all messages are properly typed and validated
 */

class SafeWebSocket {
    constructor(url) {
        this.url = url;
        this.ws = null;
        this.messageHandlers = new Map();
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.reconnectDelay = 1000;
    }

    /**
     * Connect to WebSocket with automatic reconnection
     */
    connect() {
        try {
            this.ws = new WebSocket(this.url);
            
            this.ws.onopen = () => {
                console.log('WebSocket connected');
                this.reconnectAttempts = 0;
                this.onConnect();
            };

            this.ws.onmessage = (event) => {
                this.handleMessage(event.data);
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.onError(error);
            };

            this.ws.onclose = () => {
                console.log('WebSocket closed');
                this.onDisconnect();
                this.attemptReconnect();
            };
        } catch (error) {
            console.error('Failed to create WebSocket:', error);
            this.attemptReconnect();
        }
    }

    /**
     * Handle incoming messages with type validation
     */
    handleMessage(data) {
        try {
            const message = JSON.parse(data);
            
            // Validate message structure
            if (!message.type) {
                console.error('Invalid message: missing type', message);
                return;
            }

            // Handle different message types
            switch (message.type) {
                case 'market_update':
                    this.handleMarketUpdate(message.data);
                    break;
                case 'position_update':
                    this.handlePositionUpdate(message.data);
                    break;
                case 'error':
                    this.handleError(message.data);
                    break;
                case 'snapshot':
                    this.handleSnapshot(message.data);
                    break;
                default:
                    console.warn('Unknown message type:', message.type);
            }

            // Call registered handlers
            const handlers = this.messageHandlers.get(message.type) || [];
            handlers.forEach(handler => {
                try {
                    handler(message.data);
                } catch (error) {
                    console.error('Handler error:', error);
                }
            });
        } catch (error) {
            console.error('Failed to parse WebSocket message:', error);
        }
    }

    /**
     * Handle market update with type validation
     */
    handleMarketUpdate(data) {
        if (!data || !data.market_id) {
            console.error('Invalid market update:', data);
            return;
        }

        // Ensure market_id is handled as string
        data.market_id = String(data.market_id);
        
        // Convert numeric fields to BigInt-compatible format
        if (data.total_volume) {
            data.total_volume = String(data.total_volume);
        }
        if (data.total_liquidity) {
            data.total_liquidity = String(data.total_liquidity);
        }

        console.log('Market update:', data);
    }

    /**
     * Handle position update with type validation
     */
    handlePositionUpdate(data) {
        const validation = TypeValidator.validate(data, PositionType);
        if (!validation.isValid) {
            console.error('Invalid position update:', validation.errors);
            return;
        }

        const position = TypeValidator.createPosition(data);
        if (position) {
            console.log('Position update:', position);
        }
    }

    /**
     * Send type-safe message
     */
    send(type, data) {
        if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
            console.error('WebSocket not connected');
            return false;
        }

        try {
            const message = {
                type,
                data,
                timestamp: Date.now()
            };

            // Validate specific message types
            if (type === 'subscribe') {
                if (!data.market_id) {
                    console.error('Subscribe requires market_id');
                    return false;
                }
                // Ensure market_id is string
                data.market_id = String(data.market_id);
            }

            this.ws.send(JSON.stringify(message));
            return true;
        } catch (error) {
            console.error('Failed to send message:', error);
            return false;
        }
    }

    /**
     * Subscribe to market updates
     */
    subscribeToMarket(marketId) {
        return this.send('subscribe', {
            market_id: String(marketId),
            channel: 'market_updates'
        });
    }

    /**
     * Subscribe to position updates
     */
    subscribeToPositions(wallet) {
        return this.send('subscribe', {
            wallet,
            channel: 'position_updates'
        });
    }

    /**
     * Register a message handler
     */
    on(messageType, handler) {
        if (!this.messageHandlers.has(messageType)) {
            this.messageHandlers.set(messageType, []);
        }
        this.messageHandlers.get(messageType).push(handler);
    }

    /**
     * Remove a message handler
     */
    off(messageType, handler) {
        const handlers = this.messageHandlers.get(messageType);
        if (handlers) {
            const index = handlers.indexOf(handler);
            if (index > -1) {
                handlers.splice(index, 1);
            }
        }
    }

    /**
     * Attempt to reconnect
     */
    attemptReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnection attempts reached');
            return;
        }

        this.reconnectAttempts++;
        const delay = this.reconnectDelay * this.reconnectAttempts;

        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        
        setTimeout(() => {
            this.connect();
        }, delay);
    }

    /**
     * Close the WebSocket connection
     */
    close() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
    }

    // Override these methods for custom behavior
    onConnect() {}
    onDisconnect() {}
    onError(error) {}
    handleError(data) {
        console.error('Server error:', data);
    }
    handleSnapshot(data) {
        console.log('Snapshot received:', data);
    }
}

// Export for use
if (typeof module !== 'undefined' && module.exports) {
    module.exports = SafeWebSocket;
} else {
    window.SafeWebSocket = SafeWebSocket;
}