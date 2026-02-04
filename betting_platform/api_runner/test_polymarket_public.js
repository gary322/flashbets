const axios = require('axios');

async function testPolymarketPublicAPI() {
    console.log('Testing Polymarket public API endpoints...\n');
    
    try {
        // Test 1: Get markets (public endpoint)
        console.log('1. Testing public markets endpoint...');
        const marketsResponse = await axios.get('https://clob.polymarket.com/markets', {
            params: {
                limit: 5,
                active: true
            }
        });
        
        console.log(`✅ Markets endpoint working!`);
        console.log('Response type:', typeof marketsResponse.data);
        console.log('Response keys:', Object.keys(marketsResponse.data || {}));
        
        // Handle different response formats
        const markets = Array.isArray(marketsResponse.data) ? marketsResponse.data : 
                       marketsResponse.data.markets || 
                       marketsResponse.data.data || 
                       [];
        
        console.log(`Found ${markets.length} markets`);
        
        if (markets.length > 0) {
            console.log('\nFirst market:');
            const market = markets[0];
            console.log('Full market object:', JSON.stringify(market, null, 2).substring(0, 500) + '...');
        }
        
        // Test 2: Get specific market
        if (markets.length > 0) {
            const conditionId = markets[0].condition_id || markets[0].conditionId || markets[0].id;
            console.log(`\n2. Testing market details for condition_id: ${conditionId}...`);
            
            try {
                const marketDetail = await axios.get(`https://clob.polymarket.com/markets/${conditionId}`);
                console.log('✅ Market detail endpoint working!');
            } catch (e) {
                console.log('❌ Market detail endpoint failed:', e.message);
            }
        }
        
        // Test 3: Check if we can use gamma-api (alternative endpoint)
        console.log('\n3. Testing gamma-api endpoint...');
        try {
            const gammaResponse = await axios.get('https://gamma-api.polymarket.com/markets', {
                params: { limit: 5 }
            });
            console.log('✅ Gamma API working!');
        } catch (e) {
            console.log('❌ Gamma API not accessible (may require auth)');
        }
        
        console.log('\n✅ Public API is accessible! We can fetch real Polymarket data.');
        console.log('\nNote: API key is only needed for trading operations, not for reading market data.');
        
        return marketsResponse.data;
        
    } catch (error) {
        console.error('❌ Error accessing Polymarket API:');
        console.error('Status:', error.response?.status);
        console.error('Message:', error.response?.data || error.message);
    }
}

// Run the test
testPolymarketPublicAPI();