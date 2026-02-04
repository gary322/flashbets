"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.TradingView = void 0;
const react_1 = __importStar(require("react"));
const styled_1 = __importDefault(require("@emotion/styled"));
const usePolymarketWebSocket_1 = require("../hooks/usePolymarketWebSocket");
const useMarkets_1 = require("../../hooks/useMarkets");
const LeverageSlider_1 = require("../components/trading/LeverageSlider");
const MarketSelector_1 = require("../components/trading/MarketSelector");
const PositionManager_1 = require("../components/trading/PositionManager");
const ChainBuilder_1 = require("../components/trading/ChainBuilder");
const RiskMetrics_1 = require("../components/trading/RiskMetrics");
const BlurCard_1 = require("../components/core/BlurCard");
const TPSDisplay_1 = require("../components/dashboard/TPSDisplay");
// Styled Components
const ViewContainer = styled_1.default.div `
  min-height: 100vh;
  background: ${props => props.theme.colors.background.primary};
  color: ${props => props.theme.colors.text.primary};
`;
const TradingGrid = styled_1.default.div `
  display: grid;
  grid-template-columns: 300px 1fr 350px;
  gap: 24px;
  height: 100vh;
  padding: 24px;
  
  @media (max-width: 1280px) {
    grid-template-columns: 1fr;
    height: auto;
  }
`;
const Panel = styled_1.default.section `
  display: flex;
  flex-direction: column;
`;
const MarketPanel = (0, styled_1.default)(Panel) `
  background: ${props => props.theme.colors.background.secondary};
  border-radius: 12px;
  padding: 20px;
  overflow: hidden;
`;
const TradingPanel = (0, styled_1.default)(Panel) `
  display: flex;
  flex-direction: column;
  gap: 20px;
`;
const PositionsPanel = (0, styled_1.default)(Panel) `
  background: ${props => props.theme.colors.background.secondary};
  border-radius: 12px;
  overflow: hidden;
`;
const MarketHeader = styled_1.default.div `
  padding: 24px;
  background: ${props => props.theme.colors.background.secondary};
  border-radius: 12px;
  margin-bottom: 20px;
`;
const MarketTitle = styled_1.default.h2 `
  font-size: 24px;
  font-weight: 700;
  margin: 0 0 8px 0;
`;
const MarketPrice = styled_1.default.div `
  display: flex;
  align-items: baseline;
  gap: 16px;
`;
const CurrentPrice = styled_1.default.span `
  font-size: 36px;
  font-weight: 900;
  font-family: ${props => props.theme.typography.fonts.mono};
`;
const PriceChange = styled_1.default.span `
  font-size: 18px;
  color: ${props => props.positive ?
    props.theme.colors.accent.primary :
    props.theme.colors.accent.secondary};
`;
const TradingCard = (0, styled_1.default)(BlurCard_1.BlurCard) `
  padding: 24px;
`;
const ChainToggle = styled_1.default.button `
  width: 100%;
  padding: 12px;
  background: ${props => props.theme.colors.background.tertiary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  color: ${props => props.theme.colors.text.primary};
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 200ms ease;
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.leverage};
  }
`;
const TradeActions = styled_1.default.div `
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-top: 24px;
`;
const TradeButton = styled_1.default.button `
  padding: 16px;
  border: none;
  border-radius: 8px;
  font-size: 16px;
  font-weight: 700;
  cursor: pointer;
  transition: all 200ms ease;
  
  ${props => props.variant === 'buy' ? `
    background: ${props.theme.colors.accent.primary};
    color: ${props.theme.colors.background.primary};
    
    &:hover {
      background: ${props.theme.colors.accent.primary}DD;
      transform: translateY(-2px);
    }
  ` : `
    background: ${props.theme.colors.accent.secondary};
    color: ${props.theme.colors.text.primary};
    
    &:hover {
      background: ${props.theme.colors.accent.secondary}DD;
      transform: translateY(-2px);
    }
  `}
  
  &:active {
    transform: translateY(0);
  }
`;
const EmptyState = styled_1.default.div `
  display: flex;
  align-items: center;
  justify-content: center;
  height: 400px;
  color: ${props => props.theme.colors.text.tertiary};
  font-size: 16px;
`;
const TradingView = () => {
    var _a, _b;
    const { markets, loading: marketsLoading, error: marketsError } = (0, useMarkets_1.useMarkets)();
    const [selectedMarket, setSelectedMarket] = (0, react_1.useState)(null);
    const [leverage, setLeverage] = (0, react_1.useState)(10);
    const [chainSteps, setChainSteps] = (0, react_1.useState)([]);
    const [showChainBuilder, setShowChainBuilder] = (0, react_1.useState)(false);
    const [positions, setPositions] = (0, react_1.useState)([]);
    const { prices, subscribe, unsubscribe, isConnected } = (0, usePolymarketWebSocket_1.usePolymarketWebSocket)();
    // Default coverage for liquidation calculations
    const coverage = 1.5;
    // Calculate effective leverage with chaining
    const effectiveLeverage = (0, react_1.useMemo)(() => {
        if (chainSteps.length === 0)
            return leverage;
        return chainSteps.reduce((eff, step) => {
            return eff * (1 + step.multiplier);
        }, leverage);
    }, [leverage, chainSteps]);
    // Real-time liquidation price calculation
    const liquidationPrice = (0, react_1.useMemo)(() => {
        var _a;
        if (!selectedMarket)
            return null;
        const marginRatio = 1 / coverage;
        const entryPrice = ((_a = prices.get(selectedMarket.id)) === null || _a === void 0 ? void 0 : _a.price) || selectedMarket.lastPrice;
        return entryPrice * (1 - (marginRatio / effectiveLeverage));
    }, [selectedMarket, effectiveLeverage, prices, coverage]);
    // Subscribe to market updates when selected
    (0, react_1.useEffect)(() => {
        if (selectedMarket) {
            subscribe(selectedMarket.id);
            return () => unsubscribe(selectedMarket.id);
        }
    }, [selectedMarket, subscribe, unsubscribe]);
    const handleClosePosition = (positionId) => {
        setPositions(prev => prev.filter(p => p.id !== positionId));
    };
    const handleModifyPosition = (positionId, changes) => {
        // Implement position modification logic
        console.log('Modify position:', positionId, changes);
    };
    const handleTrade = (side) => {
        if (!selectedMarket)
            return;
        // Implement trade execution logic
        console.log('Execute trade:', {
            market: selectedMarket.id,
            side,
            leverage: effectiveLeverage,
            chainSteps
        });
    };
    return (<ViewContainer>
      <TradingGrid>
        {/* Market Selection Panel */}
        <MarketPanel>
          {marketsError ? (<EmptyState>Error loading markets: {marketsError}</EmptyState>) : (<MarketSelector_1.MarketSelector markets={markets} selectedMarket={selectedMarket} onSelect={setSelectedMarket} prices={prices} searchPlaceholder="Search markets..."/>)}
        </MarketPanel>

        {/* Main Trading Panel */}
        <TradingPanel>
          {/* TPS Display at top of trading panel */}
          <TPSDisplay_1.TPSDisplay />
          
          {selectedMarket ? (<>
              <MarketHeader>
                <MarketTitle>{selectedMarket.name}</MarketTitle>
                <MarketPrice>
                  <CurrentPrice>
                    {((((_a = prices.get(selectedMarket.id)) === null || _a === void 0 ? void 0 : _a.price) || selectedMarket.lastPrice) * 100).toFixed(1)}%
                  </CurrentPrice>
                  <PriceChange positive={selectedMarket.change24h >= 0}>
                    {selectedMarket.change24h >= 0 ? '+' : ''}{selectedMarket.change24h.toFixed(2)}%
                  </PriceChange>
                </MarketPrice>
              </MarketHeader>

              <TradingCard>
                <LeverageSlider_1.LeverageSlider value={leverage} onChange={setLeverage} max={100} effectiveLeverage={effectiveLeverage} showWarnings={true} coverage={coverage}/>

                <ChainToggle onClick={() => setShowChainBuilder(!showChainBuilder)}>
                  <span>Leverage Chaining</span>
                  <span style={{
                color: effectiveLeverage > leverage ? '#FFB800' : '#6B7280',
                fontFamily: 'monospace'
            }}>
                    {effectiveLeverage > leverage ? `+${((effectiveLeverage / leverage - 1) * 100).toFixed(0)}% boost` : 'Off'}
                  </span>
                </ChainToggle>

                {showChainBuilder && (<ChainBuilder_1.ChainBuilder steps={chainSteps} onChange={setChainSteps} maxSteps={5} verseId={selectedMarket.verseId}/>)}

                <RiskMetrics_1.RiskMetrics leverage={effectiveLeverage} liquidationPrice={liquidationPrice} entryPrice={((_b = prices.get(selectedMarket.id)) === null || _b === void 0 ? void 0 : _b.price) || selectedMarket.lastPrice} marketVolatility={selectedMarket.volatility}/>

                <TradeActions>
                  <TradeButton variant="buy" onClick={() => handleTrade('buy')}>
                    Buy / Long
                  </TradeButton>
                  <TradeButton variant="sell" onClick={() => handleTrade('sell')}>
                    Sell / Short
                  </TradeButton>
                </TradeActions>
              </TradingCard>
            </>) : (<EmptyState>Select a market to start trading</EmptyState>)}
        </TradingPanel>

        {/* Positions Panel */}
        <PositionsPanel>
          <PositionManager_1.PositionManager positions={positions} prices={prices} onClose={handleClosePosition} onModify={handleModifyPosition}/>
        </PositionsPanel>
      </TradingGrid>
    </ViewContainer>);
};
exports.TradingView = TradingView;
