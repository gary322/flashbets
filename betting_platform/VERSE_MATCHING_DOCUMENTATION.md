# Verse Matching System Documentation

## Overview
The verse matching system groups ~21,000 real Polymarket markets into ~400 pre-defined verse categories. Each verse represents a thematic grouping (e.g., "Presidential Elections", "NFL Games", "Crypto Prices") that multiple related markets share.

## Architecture

### Key Components

1. **verse_catalog.rs**
   - Contains exactly 400 pre-defined verses in a hierarchical structure
   - Uses lazy_static for efficient initialization
   - Organized by category (Politics, Sports, Crypto, Economics, etc.)

2. **verse_generator.rs**
   - Matches markets to verses from the catalog
   - Extracts keywords from market titles
   - No longer generates verses dynamically

3. **find_verses_for_market()**
   - Core matching algorithm
   - Detects category from title if category is generic
   - Returns hierarchical verses (Level 1-4)

## Hierarchical Structure

### Leverage Levels
- **Level 1** (1.2x - 1.8x): Broad categories (Low risk)
- **Level 2** (2.0x - 3.8x): Specific subcategories (Medium risk)  
- **Level 3** (3.0x - 4.5x): Detailed market types (High risk)
- **Level 4** (5.0x - 5.8x): Ultra-specific markets (Very high risk)

### Example Hierarchy
```
Politics (Level 1, 1.5x)
├── 2024 US Elections (Level 2, 2.5x)
│   ├── Biden 2024 Campaign (Level 3, 3.2x)
│   └── Trump 2024 Campaign (Level 3, 3.2x)
└── Presidential Approval Ratings (Level 2, 2.0x)
    └── Biden Approval Ratings (Level 3, 3.0x)
```

## Category Detection

The system automatically detects market categories from titles when the category is "General" or empty:

### Politics Detection
Keywords: biden, trump, election, president, democrat, republican, approval, senate, congress, governor, policy, government, fivethirtyeight, 538, poll, rating

### Crypto Detection
Keywords: btc, bitcoin, eth, ethereum, sol, solana, crypto, defi, token, blockchain

### Sports Detection
Keywords: nfl, nba, mlb, soccer, football, basketball, baseball, sports, game, match, championship, finals, super bowl, world cup

### Economics Detection
Keywords: fed, inflation, gdp, recession, unemployment, economy

## Implementation Details

### Verse Matching Algorithm

```rust
pub fn find_verses_for_market(
    market_title: &str,
    market_category: &str, 
    keywords: &[String]
) -> Vec<&'static GeneratedVerse> {
    // 1. Detect category from title if generic
    let detected_category = detect_category(market_title, market_category);
    
    // 2. Add category verse (Level 1)
    add_category_verse(detected_category);
    
    // 3. Find specific verses based on content
    find_specific_verses(market_title, detected_category, keywords);
    
    // 4. Add at least 3 general verses if needed
    ensure_minimum_verses();
}
```

### Special Cases

#### Biden Approval Markets
- Detected by: "biden" + "approval" or "biden" + "rating" or "fivethirtyeight"
- Receives:
  - Political Markets (Level 1)
  - 2024 US Elections (Level 2)
  - Presidential Approval Ratings (Level 2)
  - Biden Approval Ratings (Level 3)

#### FiveThirtyEight Markets
- Special verse: "FiveThirtyEight Forecasts" (Level 3, 4.2x)
- Triggered by "538" or "fivethirtyeight" in title

## API Integration

### Polymarket Markets Endpoint
```javascript
// Markets are fetched with verses included
GET /api/polymarket/markets

// Each market includes:
{
  "title": "Market title",
  "category": "Category", 
  "verses": [
    {
      "id": "verse_id",
      "name": "Verse Name",
      "level": 1,
      "multiplier": 1.5,
      "category": "Politics"
    }
  ]
}
```

### Test Endpoint
```bash
POST /api/test/verse-match
{
  "title": "Market title",
  "category": "Category",
  "description": "Description"
}
```

## UI Integration

### Market Display
- Markets show their assigned verses in the UI
- Users can select verses for leverage multipliers
- Verses are preserved when markets are selected

### Key Files Modified
- `/api_runner/src/verse_catalog.rs` - Pre-defined verse catalog
- `/api_runner/src/verse_generator.rs` - Matching logic
- `/programs/betting_platform_native/ui_demo/index.html` - Fixed debugVerses()
- `/programs/betting_platform_native/ui_demo/platform_main.js` - Preserve verses

## Testing

### Test Scripts
- `test_biden_verses.sh` - Tests Biden approval market verses
- `test_verse_matching_directly.sh` - Direct API testing
- `user_journey_test.html` - End-to-end user journey tests

### Verification Steps
1. Markets receive appropriate category-specific verses
2. Biden approval markets get politics verses, not generic
3. Category detection works when category is "General"
4. Minimum 3 verses assigned to each market
5. Hierarchical levels properly assigned

## Performance Considerations

- Verse catalog uses lazy_static for one-time initialization
- HashMap lookups are O(1) for verse retrieval
- Keyword extraction is optimized with pre-defined replacements
- No database queries needed for verse assignment

## Future Enhancements

1. Machine learning for better keyword extraction
2. Dynamic verse weighting based on market liquidity
3. User-specific verse recommendations
4. Verse performance analytics