# Quantum Betting Platform UI Demo

## 🚀 Quick Start - Run the UI

### Option 1: Using the provided script
```bash
./run_server.sh
```

### Option 2: Using Python directly
```bash
python3 -m http.server 8080
```

### Option 3: Using Node.js
```bash
npx http-server -p 8080
```

Then open your browser to: **http://localhost:8080**

## 📱 Complete UI Pages

### 1. **Landing Page** - http://localhost:8080/index.html
- Professional hero section with gradient backgrounds
- Feature showcase grid
- Live market previews
- Technology stack display
- Call-to-action sections

### 2. **Preview Page** - http://localhost:8080/preview.html
- Overview of all implemented features
- Links to all pages
- Implementation status
- Technical specifications

### 3. **Dashboard** - http://localhost:8080/app/dashboard.html
- Portfolio overview with total value, P&L, win rate
- Quick action buttons for common tasks
- Trending markets with real-time price updates
- Active positions table
- DeFi earnings overview
- Recent activity feed

### 4. **Market Creation Wizard** - http://localhost:8080/app/create-market.html
**5-Step Process:**
- Step 1: Basic Information (question, description, category)
- Step 2: **Verse Selection** (search and select from 32-level hierarchy)
- Step 3: AMM Type Selection (Binary, Multiple, Continuous, Quantum)
- Step 4: Market Parameters (resolution, liquidity, fees)
- Step 5: Review & Deploy

### 5. **Verse Management** - http://localhost:8080/app/verses.html
- Visual 32-level verse tree
- Create new verses with permissions
- Set access levels (Public/Restricted/Private)
- Configure verse fees
- View verse analytics

### 6. **Markets Browser** - http://localhost:8080/app/markets.html
- Advanced filtering (category, type, status, verse)
- Featured markets section
- Grid/List view toggle
- Quick bet functionality
- Quantum market displays

### 7. **Trading Terminal** - http://localhost:8080/app/trading.html
- Real-time order book
- Price charts with indicators
- Order entry with up to 100x leverage
- Position management
- Open orders tracking
- Market selector

### 8. **Portfolio Management** - http://localhost:8080/app/portfolio.html
- Total portfolio value and P&L
- Performance history chart
- Active positions with real-time P&L
- Trade history
- Risk management metrics
- Performance by category

### 9. **DeFi Hub** - http://localhost:8080/app/defi.html
- MMT Staking (18.7% APY)
- Staking tiers (Bronze to Platinum)
- Liquidity pools with APY display
- Flash loans interface
- Yield farming opportunities
- Auto-yield strategies

## 🎨 Design Features

### Color Scheme (Professional Blue Theme)
- Primary: #1D9BF0 (Professional Blue)
- Success: #00D084 (Green)
- Danger: #F91880 (Red)
- Warning: #FBBD23 (Amber)
- Background: #0F1419 (Charcoal)

### Key UI Features Implemented
- ✅ Users can add verses to markets (Step 2 of wizard)
- ✅ Professional color scheme (no cyan/magenta)
- ✅ Complete product UI with all features
- ✅ Mobile-responsive design
- ✅ Dark theme with excellent contrast
- ✅ Smooth transitions and hover effects
- ✅ Real-time price simulations

## 🔧 Technical Details

- **Smart Contracts**: 92 Native Solana programs
- **Program ID**: HKTkR5ubMM2bpjdhEo3auZsF8QAqKg6MZR5iWTosGPca
- **Framework**: Native Solana (No Anchor)
- **UI Framework**: Pure HTML/CSS/JavaScript
- **Verse Levels**: 32-level hierarchy system
- **AMM Types**: LMSR, PM-AMM, L2-AMM
- **Max Leverage**: 100x
- **TPS Capacity**: 5,250

## 📸 What You'll See

When you run the server and open the UI, you'll see:

1. **Professional Landing Page**: Modern design with smooth animations
2. **Interactive Dashboard**: Real-time updates and portfolio tracking
3. **Market Creation**: Step-by-step wizard with verse selection
4. **Verse Tree**: Visual hierarchy of all 32 levels
5. **Trading Terminal**: Professional trading interface
6. **DeFi Features**: Staking, liquidity, and farming

## 🎯 User Flows Demonstrated

### Creating a Market with Verse Selection:
1. Click "Create Market" from dashboard
2. Fill in market details
3. **Select verse location** from the tree
4. Choose AMM type
5. Configure parameters
6. Review and deploy

### Managing Verses:
1. Go to Verses page
2. View 32-level hierarchy
3. Click "Create Verse"
4. Set permissions and fees
5. Deploy new verse

### Trading Flow:
1. Browse markets
2. Open trading terminal
3. View order book
4. Place orders with leverage
5. Manage positions

## 🚦 Next Steps

After running the server:
1. Start at the landing page or preview page
2. Click "Launch App" to go to dashboard
3. Explore all features through the navigation tabs
4. Try creating a market to see verse selection
5. Check out the trading terminal
6. Explore DeFi features

The UI is fully interactive with simulated real-time data!