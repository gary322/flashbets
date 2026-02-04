/**
 * Artillery WebSocket Processor
 * Handles custom logic for WebSocket load testing
 */

module.exports = {
    beforeScenario: beforeScenario,
    afterScenario: afterScenario,
    generateRandomString: generateRandomString,
    generateRandomNumber: generateRandomNumber,
    processMessage: processMessage,
    validateResponse: validateResponse
};

// Store connection state
const connections = new Map();
const messageLatencies = new Map();

/**
 * Called before each scenario starts
 */
function beforeScenario(requestParams, context, ee, next) {
    // Initialize connection tracking
    context.vars.connectionId = generateRandomString(16);
    context.vars.messageCount = 0;
    context.vars.errorCount = 0;
    context.vars.startTime = Date.now();
    
    connections.set(context.vars.connectionId, {
        connected: false,
        subscriptions: new Set(),
        pendingMessages: new Map()
    });
    
    // Set up WebSocket event handlers
    context.ws.on('message', (data) => {
        processMessage(data, context, ee);
    });
    
    context.ws.on('error', (error) => {
        context.vars.errorCount++;
        ee.emit('customStat', 'Connection Drops', 1);
        console.error(`WebSocket error: ${error.message}`);
    });
    
    context.ws.on('close', () => {
        connections.delete(context.vars.connectionId);
    });
    
    return next();
}

/**
 * Called after each scenario completes
 */
function afterScenario(requestParams, context, ee, next) {
    const duration = Date.now() - context.vars.startTime;
    const messagesPerSecond = context.vars.messageCount / (duration / 1000);
    
    ee.emit('customStat', 'Messages Per Second', messagesPerSecond);
    
    // Clean up
    connections.delete(context.vars.connectionId);
    messageLatencies.delete(context.vars.connectionId);
    
    return next();
}

/**
 * Process incoming WebSocket messages
 */
function processMessage(data, context, ee) {
    context.vars.messageCount++;
    
    try {
        const message = JSON.parse(data);
        const connection = connections.get(context.vars.connectionId);
        
        // Track message latency
        if (message.id && connection.pendingMessages.has(message.id)) {
            const latency = Date.now() - connection.pendingMessages.get(message.id);
            ee.emit('customStat', 'WebSocket Message Latency', latency);
            connection.pendingMessages.delete(message.id);
        }
        
        // Process different message types
        switch (message.type) {
            case 'connected':
                connection.connected = true;
                ee.emit('customStat', 'Subscription Success Rate', 1);
                break;
                
            case 'subscribed':
                if (message.channel) {
                    connection.subscriptions.add(message.channel);
                }
                break;
                
            case 'market_update':
            case 'position_update':
            case 'order_update':
                // Validate update structure
                validateResponse(message, context, ee);
                break;
                
            case 'error':
                context.vars.errorCount++;
                console.error(`WebSocket error message: ${message.message}`);
                break;
        }
        
    } catch (error) {
        console.error(`Failed to process message: ${error.message}`);
        context.vars.errorCount++;
    }
}

/**
 * Validate response message structure
 */
function validateResponse(message, context, ee) {
    const requiredFields = {
        market_update: ['market_id', 'price', 'volume', 'timestamp'],
        position_update: ['position_id', 'pnl', 'status', 'timestamp'],
        order_update: ['order_id', 'status', 'filled_amount', 'timestamp']
    };
    
    const required = requiredFields[message.type];
    if (!required) return;
    
    const missingFields = required.filter(field => !(field in message));
    if (missingFields.length > 0) {
        console.error(`Missing required fields in ${message.type}: ${missingFields.join(', ')}`);
        context.vars.errorCount++;
    }
}

/**
 * Generate random string
 */
function generateRandomString(length = 8) {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    let result = '';
    for (let i = 0; i < length; i++) {
        result += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return result;
}

/**
 * Generate random number
 */
function generateRandomNumber(min, max) {
    return Math.floor(Math.random() * (max - min + 1)) + min;
}

/**
 * Custom function to track message sending
 */
module.exports.beforeRequest = function(requestParams, context, ee, next) {
    // Add message ID for latency tracking
    if (requestParams.json && !requestParams.json.id) {
        requestParams.json.id = generateRandomString(12);
        
        const connection = connections.get(context.vars.connectionId);
        if (connection) {
            connection.pendingMessages.set(requestParams.json.id, Date.now());
        }
    }
    
    return next();
};