import React, { useState, useEffect, useCallback, useMemo } from 'react';
import styled from '@emotion/styled';
import { usePolymarketWebSocket } from '../hooks/usePolymarketWebSocket';
import { useMarkets } from '../../hooks/useMarkets';
import { LeverageSlider } from '../components/trading/LeverageSlider';
import { MarketSelector } from '../components/trading/MarketSelector';
import { PositionManager } from '../components/trading/PositionManager';
import { ChainBuilder } from '../components/trading/ChainBuilder';
import { RiskMetrics } from '../components/trading/RiskMetrics';
import { BlurCard } from '../components/core/BlurCard';
import { TPSDisplay } from '../components/dashboard/TPSDisplay';
import { Market, ChainStep, Position } from '../types';

// Styled Components
const ViewContainer = styled.div`
  min-height: 100vh;
  background: ${props => props.theme.colors.background.primary};
  color: ${props => props.theme.colors.text.primary};
`;

const TradingGrid = styled.div`
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

const Panel = styled.section`
  display: flex;
  flex-direction: column;
`;

const MarketPanel = styled(Panel)`
  background: ${props => props.theme.colors.background.secondary};
  border-radius: 12px;
  padding: 20px;
  overflow: hidden;
`;

const TradingPanel = styled(Panel)`
  display: flex;
  flex-direction: column;
  gap: 20px;
`;

const PositionsPanel = styled(Panel)`
  background: ${props => props.theme.colors.background.secondary};
  border-radius: 12px;
  overflow: hidden;
`;

const MarketHeader = styled.div`
  padding: 24px;
  background: ${props => props.theme.colors.background.secondary};
  border-radius: 12px;
  margin-bottom: 20px;
`;

const MarketTitle = styled.h2`
  font-size: 24px;
  font-weight: 700;
  margin: 0 0 8px 0;
`;

const MarketPrice = styled.div`
  display: flex;
  align-items: baseline;
  gap: 16px;
`;

const CurrentPrice = styled.span`
  font-size: 36px;
  font-weight: 900;
  font-family: ${props => props.theme.typography.fonts.mono};
`;

const PriceChange = styled.span<{ positive: boolean }>`
  font-size: 18px;
  color: ${props => props.positive ? 
    props.theme.colors.accent.primary : 
    props.theme.colors.accent.secondary};
`;

const TradingCard = styled(BlurCard)`
  padding: 24px;
`;

const ChainToggle = styled.button`
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

const TradeActions = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-top: 24px;
`;

const TradeButton = styled.button<{ variant: 'buy' | 'sell' }>`
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

const EmptyState = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
  height: 400px;
  color: ${props => props.theme.colors.text.tertiary};
  font-size: 16px;
`;

export const TradingView: React.FC = () => {
  const { markets, loading: marketsLoading, error: marketsError } = useMarkets();
  const [selectedMarket, setSelectedMarket] = useState<Market | null>(null);
  const [leverage, setLeverage] = useState(10);
  const [chainSteps, setChainSteps] = useState<ChainStep[]>([]);
  const [showChainBuilder, setShowChainBuilder] = useState(false);
  const [positions, setPositions] = useState<Position[]>([]);
  
  const { prices, subscribe, unsubscribe, isConnected } = usePolymarketWebSocket();

  // Default coverage for liquidation calculations
  const coverage = 1.5;

  // Calculate effective leverage with chaining
  const effectiveLeverage = useMemo(() => {
    if (chainSteps.length === 0) return leverage;

    return chainSteps.reduce((eff, step) => {
      return eff * (1 + step.multiplier);
    }, leverage);
  }, [leverage, chainSteps]);

  // Real-time liquidation price calculation
  const liquidationPrice = useMemo(() => {
    if (!selectedMarket) return null;

    const marginRatio = 1 / coverage;
    const entryPrice = prices.get(selectedMarket.id)?.price || selectedMarket.lastPrice;

    return entryPrice * (1 - (marginRatio / effectiveLeverage));
  }, [selectedMarket, effectiveLeverage, prices, coverage]);

  // Subscribe to market updates when selected
  useEffect(() => {
    if (selectedMarket) {
      subscribe(selectedMarket.id);
      return () => unsubscribe(selectedMarket.id);
    }
  }, [selectedMarket, subscribe, unsubscribe]);

  const handleClosePosition = (positionId: string) => {
    setPositions(prev => prev.filter(p => p.id !== positionId));
  };

  const handleModifyPosition = (positionId: string, changes: Partial<Position>) => {
    // Implement position modification logic
    console.log('Modify position:', positionId, changes);
  };

  const handleTrade = (side: 'buy' | 'sell') => {
    if (!selectedMarket) return;
    
    // Implement trade execution logic
    console.log('Execute trade:', {
      market: selectedMarket.id,
      side,
      leverage: effectiveLeverage,
      chainSteps
    });
  };

  return (
    <ViewContainer>
      <TradingGrid>
        {/* Market Selection Panel */}
        <MarketPanel>
          {marketsError ? (
            <EmptyState>Error loading markets: {marketsError}</EmptyState>
          ) : (
            <MarketSelector
              markets={markets}
              selectedMarket={selectedMarket}
              onSelect={setSelectedMarket}
              prices={prices}
              searchPlaceholder="Search markets..."
            />
          )}
        </MarketPanel>

        {/* Main Trading Panel */}
        <TradingPanel>
          {/* TPS Display at top of trading panel */}
          <TPSDisplay />
          
          {selectedMarket ? (
            <>
              <MarketHeader>
                <MarketTitle>{selectedMarket.name}</MarketTitle>
                <MarketPrice>
                  <CurrentPrice>
                    {((prices.get(selectedMarket.id)?.price || selectedMarket.lastPrice) * 100).toFixed(1)}%
                  </CurrentPrice>
                  <PriceChange positive={selectedMarket.change24h >= 0}>
                    {selectedMarket.change24h >= 0 ? '+' : ''}{selectedMarket.change24h.toFixed(2)}%
                  </PriceChange>
                </MarketPrice>
              </MarketHeader>

              <TradingCard>
                <LeverageSlider
                  value={leverage}
                  onChange={setLeverage}
                  max={100}
                  effectiveLeverage={effectiveLeverage}
                  showWarnings={true}
                  coverage={coverage}
                />

                <ChainToggle
                  onClick={() => setShowChainBuilder(!showChainBuilder)}
                >
                  <span>Leverage Chaining</span>
                  <span style={{ 
                    color: effectiveLeverage > leverage ? '#FFB800' : '#6B7280',
                    fontFamily: 'monospace'
                  }}>
                    {effectiveLeverage > leverage ? `+${((effectiveLeverage / leverage - 1) * 100).toFixed(0)}% boost` : 'Off'}
                  </span>
                </ChainToggle>

                {showChainBuilder && (
                  <ChainBuilder
                    steps={chainSteps}
                    onChange={setChainSteps}
                    maxSteps={5}
                    verseId={selectedMarket.verseId}
                  />
                )}

                <RiskMetrics
                  leverage={effectiveLeverage}
                  liquidationPrice={liquidationPrice}
                  entryPrice={prices.get(selectedMarket.id)?.price || selectedMarket.lastPrice}
                  marketVolatility={selectedMarket.volatility}
                />

                <TradeActions>
                  <TradeButton variant="buy" onClick={() => handleTrade('buy')}>
                    Buy / Long
                  </TradeButton>
                  <TradeButton variant="sell" onClick={() => handleTrade('sell')}>
                    Sell / Short
                  </TradeButton>
                </TradeActions>
              </TradingCard>
            </>
          ) : (
            <EmptyState>Select a market to start trading</EmptyState>
          )}
        </TradingPanel>

        {/* Positions Panel */}
        <PositionsPanel>
          <PositionManager
            positions={positions}
            prices={prices}
            onClose={handleClosePosition}
            onModify={handleModifyPosition}
          />
        </PositionsPanel>
      </TradingGrid>
    </ViewContainer>
  );
};