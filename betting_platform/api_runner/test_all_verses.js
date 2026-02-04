#!/usr/bin/env node

const http = require('http');
const fs = require('fs');

const API_BASE = 'http://localhost:8081';

// Test markets for each category
const testMarkets = {
    politics: [
        { title: "2024 US Presidential Election", keywords: ["election", "president", "2024", "biden", "trump"] },
        { title: "Biden Approval Rating Above 50%", keywords: ["biden", "approval", "rating", "president"] },
        { title: "GOP Wins House Majority 2024", keywords: ["republican", "gop", "house", "congress", "majority"] },
        { title: "Supreme Court Decision on Roe", keywords: ["supreme", "court", "abortion", "roe", "justice"] },
        { title: "Ukraine War Ends by 2024", keywords: ["ukraine", "war", "russia", "peace", "conflict"] }
    ],
    sports: [
        { title: "Super Bowl 2024 Winner", keywords: ["nfl", "super", "bowl", "football", "championship"] },
        { title: "NBA Finals 2024 Champion", keywords: ["nba", "basketball", "finals", "championship", "playoff"] },
        { title: "World Cup 2026 Winner", keywords: ["world", "cup", "soccer", "football", "fifa"] },
        { title: "Yankees Win World Series", keywords: ["mlb", "baseball", "yankees", "world", "series"] },
        { title: "Tiger Woods Wins Major", keywords: ["golf", "tiger", "woods", "major", "pga"] }
    ],
    finance: [
        { title: "S&P 500 Above 5000", keywords: ["sp500", "stock", "market", "index", "equity"] },
        { title: "Tesla Stock Above $300", keywords: ["tesla", "tsla", "stock", "electric", "vehicle"] },
        { title: "Fed Raises Rates in 2024", keywords: ["federal", "reserve", "interest", "rates", "inflation"] },
        { title: "Recession in 2024", keywords: ["recession", "economy", "gdp", "growth", "downturn"] },
        { title: "Gold Price Above $2500", keywords: ["gold", "precious", "metal", "commodity", "price"] }
    ],
    crypto: [
        { title: "Bitcoin Above $100k", keywords: ["bitcoin", "btc", "cryptocurrency", "price", "100k"] },
        { title: "Ethereum Flips Bitcoin", keywords: ["ethereum", "eth", "flippening", "market", "cap"] },
        { title: "US Approves Bitcoin ETF", keywords: ["bitcoin", "etf", "sec", "approval", "regulation"] },
        { title: "Solana Above $500", keywords: ["solana", "sol", "blockchain", "price", "defi"] },
        { title: "CBDC Launched by 2025", keywords: ["cbdc", "digital", "currency", "central", "bank"] }
    ],
    entertainment: [
        { title: "Oscars 2024 Best Picture", keywords: ["oscars", "academy", "awards", "movie", "film"] },
        { title: "Taylor Swift New Album #1", keywords: ["taylor", "swift", "music", "album", "billboard"] },
        { title: "Marvel Movie Tops Box Office", keywords: ["marvel", "mcu", "movie", "box", "office"] },
        { title: "Netflix Wins Most Emmys", keywords: ["netflix", "emmy", "awards", "streaming", "television"] },
        { title: "GTA 6 Breaks Sales Record", keywords: ["gta", "grand", "theft", "auto", "gaming"] }
    ],
    science: [
        { title: "SpaceX Mars Landing 2024", keywords: ["spacex", "mars", "landing", "space", "exploration"] },
        { title: "AI Wins Nobel Prize", keywords: ["ai", "artificial", "intelligence", "nobel", "prize"] },
        { title: "Fusion Power Breakthrough", keywords: ["fusion", "energy", "nuclear", "power", "breakthrough"] },
        { title: "COVID Variant Emerges", keywords: ["covid", "variant", "pandemic", "virus", "health"] },
        { title: "Quantum Computer Supremacy", keywords: ["quantum", "computer", "supremacy", "technology", "computing"] }
    ],
    technology: [
        { title: "Apple Releases AR Glasses", keywords: ["apple", "ar", "augmented", "reality", "glasses"] },
        { title: "ChatGPT 5 Released", keywords: ["chatgpt", "openai", "ai", "language", "model"] },
        { title: "Tesla Full Self Driving", keywords: ["tesla", "fsd", "autonomous", "driving", "autopilot"] },
        { title: "Meta Stock Above $400", keywords: ["meta", "facebook", "stock", "social", "media"] },
        { title: "iPhone 16 Sales Record", keywords: ["iphone", "apple", "sales", "smartphone", "record"] }
    ],
    weather: [
        { title: "Hurricane Season Active 2024", keywords: ["hurricane", "season", "atlantic", "storm", "weather"] },
        { title: "California Drought Ends", keywords: ["california", "drought", "rain", "water", "weather"] },
        { title: "Record Heat Wave 2024", keywords: ["heat", "wave", "temperature", "record", "climate"] },
        { title: "White Christmas NYC", keywords: ["white", "christmas", "snow", "nyc", "weather"] },
        { title: "El Nino Effects 2024", keywords: ["el", "nino", "weather", "pattern", "climate"] }
    ]
};

async function makeRequest(path, method = 'GET', data = null) {
    return new Promise((resolve, reject) => {
        const url = new URL(API_BASE + path);
        const options = {
            hostname: url.hostname,
            port: url.port,
            path: url.pathname + url.search,
            method: method,
            headers: {}
        };
        
        if (data) {
            options.headers['Content-Type'] = 'application/json';
        }
        
        const req = http.request(options, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    resolve(JSON.parse(body));
                } catch (e) {
                    resolve(body);
                }
            });
        });
        
        req.on('error', reject);
        
        if (data) {
            req.write(JSON.stringify(data));
        }
        
        req.end();
    });
}

async function testVerseMatching(market, category) {
    const response = await makeRequest('/api/test/verse-match', 'POST', {
        title: market.title,
        category: category,
        keywords: market.keywords
    });
    
    return {
        market: market.title,
        category: category,
        versesFound: response.count || 0,
        verses: response.matching_verses || []
    };
}

async function runAllVerseTests() {
    console.log('🔮 COMPREHENSIVE VERSE TESTING SUITE');
    console.log('=====================================\n');
    
    const results = {
        totalMarkets: 0,
        totalVerses: 0,
        byCategory: {},
        byRiskTier: {
            Low: 0,
            Medium: 0,
            High: 0,
            Extreme: 0
        },
        verseDistribution: {}
    };
    
    // First, get all verses to analyze
    const allVerses = await makeRequest('/api/verses');
    const verses = Array.isArray(allVerses) ? allVerses : (allVerses.verses || []);
    console.log(`📊 Total Verses Available: ${verses.length}\n`);
    
    // Analyze verse distribution
    verses.forEach(verse => {
        if (!results.verseDistribution[verse.category]) {
            results.verseDistribution[verse.category] = {
                total: 0,
                byRiskTier: { Low: 0, Medium: 0, High: 0, Extreme: 0 }
            };
        }
        results.verseDistribution[verse.category].total++;
        results.verseDistribution[verse.category].byRiskTier[verse.risk_tier]++;
    });
    
    // Test each market
    for (const [category, markets] of Object.entries(testMarkets)) {
        console.log(`\n📁 Testing ${category.toUpperCase()} Markets:`);
        console.log('─'.repeat(40));
        
        results.byCategory[category] = {
            markets: 0,
            totalVerses: 0,
            avgVerses: 0,
            versesByTier: { Low: 0, Medium: 0, High: 0, Extreme: 0 }
        };
        
        for (const market of markets) {
            const result = await testVerseMatching(market, category);
            results.totalMarkets++;
            results.byCategory[category].markets++;
            
            console.log(`\n📈 ${market.title}`);
            console.log(`   Found ${result.versesFound} matching verses:`);
            
            result.verses.forEach(verse => {
                console.log(`   • ${verse.name} (${verse.multiplier}x) - ${verse.risk_tier} Risk`);
                results.totalVerses++;
                results.byCategory[category].totalVerses++;
                results.byCategory[category].versesByTier[verse.risk_tier]++;
                results.byRiskTier[verse.risk_tier]++;
            });
            
            // Add delay to avoid overwhelming the server
            await new Promise(resolve => setTimeout(resolve, 100));
        }
        
        results.byCategory[category].avgVerses = 
            results.byCategory[category].totalVerses / results.byCategory[category].markets;
    }
    
    // Generate summary report
    console.log('\n\n🎯 VERSE TESTING SUMMARY REPORT');
    console.log('================================\n');
    
    console.log('📊 Overall Statistics:');
    console.log(`   • Total Markets Tested: ${results.totalMarkets}`);
    console.log(`   • Total Verse Matches: ${results.totalVerses}`);
    console.log(`   • Average Verses per Market: ${(results.totalVerses / results.totalMarkets).toFixed(2)}`);
    
    console.log('\n🎲 Risk Tier Distribution:');
    Object.entries(results.byRiskTier).forEach(([tier, count]) => {
        const percentage = ((count / results.totalVerses) * 100).toFixed(1);
        console.log(`   • ${tier} Risk: ${count} verses (${percentage}%)`);
    });
    
    console.log('\n📁 Category Analysis:');
    Object.entries(results.byCategory).forEach(([category, data]) => {
        console.log(`\n   ${category.toUpperCase()}:`);
        console.log(`   • Markets Tested: ${data.markets}`);
        console.log(`   • Total Verses: ${data.totalVerses}`);
        console.log(`   • Avg Verses/Market: ${data.avgVerses.toFixed(2)}`);
        console.log(`   • Risk Distribution:`);
        Object.entries(data.versesByTier).forEach(([tier, count]) => {
            if (count > 0) {
                console.log(`     - ${tier}: ${count}`);
            }
        });
    });
    
    console.log('\n📚 Verse Catalog Distribution:');
    Object.entries(results.verseDistribution).forEach(([category, data]) => {
        console.log(`\n   ${category}:`);
        console.log(`   • Total Verses: ${data.total}`);
        console.log(`   • By Risk Tier:`);
        Object.entries(data.byRiskTier).forEach(([tier, count]) => {
            if (count > 0) {
                console.log(`     - ${tier}: ${count}`);
            }
        });
    });
    
    // Save detailed report
    const report = {
        timestamp: new Date().toISOString(),
        results: results,
        detailedTests: testMarkets,
        versesCatalog: verses.length
    };
    
    fs.writeFileSync('verse_test_report.json', JSON.stringify(report, null, 2));
    console.log('\n\n✅ Detailed report saved to verse_test_report.json');
}

// Run the tests
runAllVerseTests().catch(console.error);