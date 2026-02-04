// WebSocket Test Script
const WebSocket = require('ws');

console.log('=== WebSocket Real-time Updates Test ===\n');

// Test standard WebSocket
console.log('1. Testing standard WebSocket connection...');
const ws = new WebSocket('ws://localhost:8081/ws');

let messageCount = 0;
const testDuration = 5000; // 5 seconds

ws.on('open', () => {
    console.log('   ✓ Connected to standard WebSocket');
    
    // Subscribe to market updates
    ws.send(JSON.stringify({
        type: 'subscribe',
        channel: 'markets'
    }));
    console.log('   ✓ Subscribed to market updates');
});

ws.on('message', (data) => {
    messageCount++;
    const message = JSON.parse(data);
    if (messageCount === 1) {
        console.log(`   ✓ First message received: ${message.type || 'unknown'}`);
    }
});

ws.on('error', (err) => {
    console.log(`   ❌ WebSocket error: ${err.message}`);
});

// Test enhanced WebSocket
console.log('\n2. Testing enhanced WebSocket connection...');
const wsV2 = new WebSocket('ws://localhost:8081/ws/v2');

let v2MessageCount = 0;

wsV2.on('open', () => {
    console.log('   ✓ Connected to enhanced WebSocket');
    
    // Subscribe to enhanced updates
    wsV2.send(JSON.stringify({
        type: 'subscribe',
        channels: ['markets', 'trades', 'orderbook']
    }));
    console.log('   ✓ Subscribed to multiple channels');
});

wsV2.on('message', (data) => {
    v2MessageCount++;
    const message = JSON.parse(data);
    if (v2MessageCount === 1) {
        console.log(`   ✓ First enhanced message received: ${message.type || 'unknown'}`);
    }
});

wsV2.on('error', (err) => {
    console.log(`   ❌ Enhanced WebSocket error: ${err.message}`);
});

// Summary after test duration
setTimeout(() => {
    console.log('\n=== SUMMARY ===');
    console.log(`✅ Standard WebSocket: ${messageCount} messages received`);
    console.log(`✅ Enhanced WebSocket: ${v2MessageCount} messages received`);
    
    if (messageCount > 0) {
        console.log('✅ Real-time updates working');
    } else {
        console.log('⚠️  No real-time updates received');
    }
    
    ws.close();
    wsV2.close();
    process.exit(0);
}, testDuration);