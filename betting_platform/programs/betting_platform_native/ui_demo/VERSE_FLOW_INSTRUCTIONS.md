# How to View the Verse Flow with Connected Arrows

The verse flow visualization with connected arrows has been implemented, but it only appears after selecting a market. Here's how to see it:

## Steps to View Verse Connections:

1. **Open the UI**: Navigate to http://localhost:8080/

2. **Search for a Market**: 
   - In the left panel, use the search box
   - Try searching for: "Bitcoin", "Trump", "AI", or "Sports"
   - Click on any market from the search results

3. **View the Verse Flow**:
   - After selecting a market, the main panel will show the market details
   - Scroll down below the "Market Outcomes" section
   - You'll see "Related Stages - Chain Your Leverage" section
   - The verse flow will display with:
     - Multiple levels (Level 1, 2, 3)
     - Connected arrows between verses
     - Animated flow lines
     - Interactive verse cards

## Features Implemented:

### Visual Layout:
- **Horizontal Flow**: Verses are arranged in levels from left to right
- **Level Labels**: Vertical labels for Level 1, 2, 3
- **Card Design**: Each verse shows name, multiplier, description, and risk level

### Interactive Elements:
- **Hover Effects**: Cards scale up and glow on hover
- **Selection**: Click verses to select them (shows checkmark)
- **Active Connections**: When connected verses are selected, the arrow becomes solid orange
- **Animated Lines**: Unselected connections show flowing dashed lines

### Connection Logic:
- Verses connect if they have progressive multipliers (increasing risk)
- Verses connect if they share common themes/keywords
- Connections update dynamically when verses are selected

### Responsive Design:
- On screens wider than 1200px: Shows flow view with arrows
- On smaller screens: Falls back to grid view

## Test Pages:

If you want to see the verse flow directly without selecting a market:
- Open http://localhost:8080/test-verse-direct.html for a standalone demo
- Open http://localhost:8080/test-verses.html for debugging tools

## Troubleshooting:

If you don't see the verses:
1. Make sure you've selected a market first
2. Ensure your browser window is wider than 1200px
3. Check the browser console for any JavaScript errors
4. Try refreshing the page and selecting a market again

The implementation is complete and functional - you just need to select a market to see it in action!