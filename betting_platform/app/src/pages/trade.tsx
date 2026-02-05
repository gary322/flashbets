import React, { useState, useEffect } from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';
import { useRouter } from 'next/router';
import { useMetaMask } from '../hooks/useMetaMask';
import { usePolymarketOrder } from '../hooks/usePolymarketOrder';

const TradeContainer = styled.div`
  max-width: 1440px;
  margin: 0 auto;
  padding: 32px 24px;
  display: flex;
  gap: 24px;
  
  @media (max-width: 1024px) {
    flex-direction: column;
  }
`;

const MainContent = styled.div`
  flex: 1;
`;

const Sidebar = styled.div`
  width: 400px;
  
  @media (max-width: 1024px) {
    width: 100%;
  }
`;

const MarketHeader = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
  margin-bottom: 24px;
`;

const MarketTitle = styled.h1`
  font-size: 28px;
  font-weight: 800;
  margin-bottom: 8px;
  color: ${props => props.theme.colors.text.primary};
`;

const MarketDescription = styled.p`
  font-size: 16px;
  color: ${props => props.theme.colors.text.secondary};
  line-height: 1.5;
`;

const ChartContainer = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
  height: 400px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 24px;
  color: ${props => props.theme.colors.text.secondary};
`;

const OrderBook = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
`;

const TradingPanel = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
  margin-bottom: 24px;
`;

const TradingTabs = styled.div`
  display: flex;
  gap: 16px;
  margin-bottom: 24px;
`;

const Tab = styled.button<{ active?: boolean }>`
  padding: 8px 16px;
  border: none;
  background: ${props => props.active ? props.theme.colors.accent.primary : 'transparent'};
  color: ${props => props.active ? '#000' : props.theme.colors.text.secondary};
  font-weight: 600;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    background: ${props => !props.active && 'rgba(0, 255, 136, 0.1)'};
  }
`;

const OutcomeSelector = styled.div`
  display: flex;
  gap: 12px;
  margin-bottom: 24px;
`;

const OutcomeButton = styled.button<{ selected?: boolean }>`
  flex: 1;
  padding: 16px;
  border-radius: 8px;
  border: 2px solid ${props => props.selected ? props.theme.colors.accent.primary : 'rgba(255, 255, 255, 0.1)'};
  background: ${props => props.selected ? 'rgba(0, 255, 136, 0.1)' : 'transparent'};
  color: ${props => props.theme.colors.text.primary};
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
  }
`;

const InputGroup = styled.div`
  margin-bottom: 20px;
`;

const Label = styled.label`
  display: block;
  margin-bottom: 8px;
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const Input = styled.input`
  width: 100%;
  padding: 12px 16px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: ${props => props.theme.colors.background.primary};
  color: ${props => props.theme.colors.text.primary};
  font-size: 16px;
  
  &:focus {
    outline: none;
    border-color: ${props => props.theme.colors.accent.primary};
  }
`;

const LeverageSlider = styled.div`
  margin-bottom: 24px;
`;

const SliderValue = styled.div`
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
`;

const Slider = styled.input`
  width: 100%;
  height: 8px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.1);
  outline: none;
  -webkit-appearance: none;
  
  &::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: ${props => props.theme.colors.accent.primary};
    cursor: pointer;
  }
  
  &::-moz-range-thumb {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: ${props => props.theme.colors.accent.primary};
    cursor: pointer;
    border: none;
  }
`;

const TradeButton = styled.button`
  width: 100%;
  padding: 16px;
  border-radius: 8px;
  border: none;
  background: ${props => props.theme.colors.accent.primary};
  color: #000;
  font-size: 18px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
  
  &:active {
    transform: translateY(0);
  }
`;

const InfoRow = styled.div`
  display: flex;
  justify-content: space-between;
  margin-bottom: 12px;
  font-size: 14px;
`;

const InfoLabel = styled.span`
  color: ${props => props.theme.colors.text.secondary};
`;

const InfoValue = styled.span`
  color: ${props => props.theme.colors.text.primary};
  font-weight: 500;
`;

interface MarketData {
  id: string;
  title: string;
  description: string;
  outcomes: Array<{
    name: string;
    price: number;
    volume: number;
    liquidity: number;
  }>;
  total_volume: number;
  total_liquidity: number;
  resolution_time?: string | number;
  source?: string;
}

const API_BASE_URL = '/api';

export default function Trade() {
  const router = useRouter();
  const { market } = router.query;
  const [selectedOutcome, setSelectedOutcome] = useState(0);
  const [amount, setAmount] = useState('');
  const [orderType, setOrderType] = useState<'buy' | 'sell'>('buy');
  const [marketData, setMarketData] = useState<MarketData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  
  const { wallet, connect } = useMetaMask();
  const { prepareOrder, signAndSubmitOrder } = usePolymarketOrder();

  useEffect(() => {
    if (market) {
      fetchMarketData(market as string);
    }
  }, [market]);

  const fetchMarketData = async (marketId: string) => {
    try {
      setLoading(true);
      setError(null);
      
      const response = await fetch(`${API_BASE_URL}/markets/${marketId}`);
      if (!response.ok) {
        throw new Error(`Failed to fetch market data: ${response.status}`);
      }
      
      const data = await response.json();

      const totalVolume = typeof data.total_volume === 'number' ? data.total_volume : 0;
      const totalLiquidity = typeof data.total_liquidity === 'number' ? data.total_liquidity : 0;

      const rawOutcomes: any[] = Array.isArray(data.outcomes) ? data.outcomes : [];
      const normalizedOutcomes =
        rawOutcomes.length > 0
          ? rawOutcomes.map((outcome, index) => ({
              name: outcome?.name || outcome?.title || `Outcome ${index + 1}`,
              price:
                typeof outcome?.price === 'number'
                  ? outcome.price
                  : 1 / Math.max(rawOutcomes.length, 2),
              volume:
                typeof outcome?.volume === 'number'
                  ? outcome.volume
                  : totalVolume / Math.max(rawOutcomes.length, 1),
              liquidity:
                typeof outcome?.liquidity === 'number'
                  ? outcome.liquidity
                  : totalLiquidity / Math.max(rawOutcomes.length, 1),
            }))
          : [
              { name: 'Yes', price: 0.5, volume: totalVolume / 2 || 0, liquidity: totalLiquidity / 2 || 0 },
              { name: 'No', price: 0.5, volume: totalVolume / 2 || 0, liquidity: totalLiquidity / 2 || 0 },
            ];

      setMarketData({
        id: String(data.id ?? marketId),
        title: data.title || 'Unknown Market',
        description: data.description || '',
        outcomes: normalizedOutcomes,
        total_volume: totalVolume,
        total_liquidity: totalLiquidity,
        resolution_time: data.resolution_time,
        source: data.source,
      });
    } catch (err) {
      console.error('Error fetching market data:', err);
      setError(err instanceof Error ? err.message : 'Failed to load market data');
      
      // Fallback mock data for demonstration
      setMarketData({
        id: marketId,
        title: '2024 US Presidential Election Winner',
        description: 'Who will win the 2024 United States presidential election?',
        outcomes: [
          { name: 'Biden', price: 0.45, volume: 1000000, liquidity: 500000 },
          { name: 'Trump', price: 0.40, volume: 1200000, liquidity: 600000 },
          { name: 'Other', price: 0.15, volume: 300000, liquidity: 150000 }
        ],
        total_volume: 2500000,
        total_liquidity: 1250000,
        source: 'demo'
      });
    } finally {
      setLoading(false);
    }
  };

  const handleTrade = async () => {
    if (!wallet.isConnected) {
      await connect();
      return;
    }

    if (!amount || parseFloat(amount) <= 0) {
      setError('Please enter a valid amount');
      return;
    }

    if (!marketData) {
      setError('Market data not loaded');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    try {
      // Prepare the order
      const preparedOrder = await prepareOrder({
        tokenId: marketData.id, // This should be the actual Polymarket token ID
        side: orderType,
        price: marketData.outcomes[selectedOutcome].price,
        size: parseFloat(amount),
        marketId: marketData.id,
        outcome: selectedOutcome
      });

      // Show confirmation dialog (you could implement a modal here)
      const confirmed = window.confirm(
        `Confirm ${preparedOrder.displayData.side} Order:\n` +
        `Market: ${preparedOrder.displayData.market}\n` +
        `Outcome: ${preparedOrder.displayData.outcome}\n` +
        `Price: ${preparedOrder.displayData.price}\n` +
        `Size: ${preparedOrder.displayData.size}\n` +
        `Fee: ${preparedOrder.displayData.fee}\n` +
        `Total: ${preparedOrder.displayData.total}`
      );

      if (!confirmed) {
        setIsSubmitting(false);
        return;
      }

      // Sign and submit the order
      const result = await signAndSubmitOrder(preparedOrder);
      
      // Success! Redirect to portfolio or show success message
      const orderId = result?.orderId || result?.order_id || result?.orderID;
      alert(`Order submitted successfully! Order ID: ${orderId ?? 'unknown'}`);
      setAmount('');
      
    } catch (err: any) {
      setError(err.message || 'Failed to submit order');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <>
      <Head>
        <title>Trade - Betting Platform</title>
        <meta name="description" content="Trade prediction markets with leverage" />
      </Head>

      <TradeContainer>
        <MainContent>
          {loading ? (
            <MarketHeader>
              <div style={{ color: '#9CA3AF' }}>Loading market data...</div>
            </MarketHeader>
          ) : error ? (
            <MarketHeader>
              <div style={{ color: '#FF3B30' }}>Error: {error}</div>
              <div style={{ color: '#9CA3AF', fontSize: '14px', marginTop: '8px' }}>
                Using fallback demo data
              </div>
            </MarketHeader>
          ) : marketData ? (
            <MarketHeader>
              <MarketTitle>{marketData.title}</MarketTitle>
              <MarketDescription>
                {marketData.description}
              </MarketDescription>
              <div style={{ marginTop: '16px', display: 'flex', gap: '24px', fontSize: '14px', color: '#9CA3AF' }}>
                <div>Volume: ${marketData.total_volume.toLocaleString()}</div>
                <div>Liquidity: ${marketData.total_liquidity.toLocaleString()}</div>
                <div>Market ID: {marketData.id}</div>
              </div>
            </MarketHeader>
          ) : null}

          <ChartContainer>
            <div>Price Chart Coming Soon</div>
          </ChartContainer>

          <OrderBook>
            <h3 style={{ marginBottom: '16px' }}>Order Book</h3>
            <div style={{ color: '#9CA3AF' }}>Order book data coming soon...</div>
          </OrderBook>
        </MainContent>

        <Sidebar>
          <TradingPanel>
            <TradingTabs>
              <Tab active={orderType === 'buy'} onClick={() => setOrderType('buy')}>
                Buy
              </Tab>
              <Tab active={orderType === 'sell'} onClick={() => setOrderType('sell')}>
                Sell
              </Tab>
            </TradingTabs>

            <OutcomeSelector>
              {marketData?.outcomes.map((outcome, index) => (
                <OutcomeButton 
                  key={index}
                  selected={selectedOutcome === index}
                  onClick={() => setSelectedOutcome(index)}
                >
                  <div>{outcome.name}</div>
                  <div style={{ fontSize: '12px', opacity: 0.7 }}>
                    {(outcome.price * 100).toFixed(1)}%
                  </div>
                </OutcomeButton>
              )) || (
                <>
                  <OutcomeButton 
                    selected={selectedOutcome === 0}
                    onClick={() => setSelectedOutcome(0)}
                  >
                    Yes
                  </OutcomeButton>
                  <OutcomeButton 
                    selected={selectedOutcome === 1}
                    onClick={() => setSelectedOutcome(1)}
                  >
                    No
                  </OutcomeButton>
                </>
              )}
            </OutcomeSelector>

            <InputGroup>
              <Label>Amount (USDC)</Label>
              <Input
                type="number"
                placeholder="0.00"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                data-testid="trade-amount"
              />
            </InputGroup>

            {/* Leverage removed - Polymarket doesn't support leverage */}

            <div style={{ marginBottom: '24px' }}>
              <InfoRow>
                <InfoLabel>Position Size</InfoLabel>
                <InfoValue>{amount ? Number(amount).toFixed(2) : '0.00'} USDC</InfoValue>
              </InfoRow>
              <InfoRow>
                <InfoLabel>Entry Price</InfoLabel>
                <InfoValue>
                  {marketData?.outcomes[selectedOutcome]?.price ? 
                    (marketData.outcomes[selectedOutcome].price * 100).toFixed(1) + '%' : 
                    '50.0%'
                  }
                </InfoValue>
              </InfoRow>
              <InfoRow>
                <InfoLabel>Fees (0.5%)</InfoLabel>
                <InfoValue>{amount ? (Number(amount) * 0.005).toFixed(2) : '0.00'} USDC</InfoValue>
              </InfoRow>
              <InfoRow>
                <InfoLabel>Outcome</InfoLabel>
                <InfoValue>
                  {marketData?.outcomes[selectedOutcome]?.name || 'Yes'}
                </InfoValue>
              </InfoRow>
            </div>

            <TradeButton 
              onClick={handleTrade}
              disabled={isSubmitting || loading}
              data-testid="trade-submit"
            >
              {isSubmitting ? 'Processing...' : 
               !wallet.isConnected ? 'Connect Wallet' :
               `${orderType === 'buy' ? 'Buy' : 'Sell'} Position`}
            </TradeButton>
          </TradingPanel>

          {error && (
            <div style={{ 
              background: 'rgba(255, 59, 48, 0.1)', 
              border: '1px solid rgba(255, 59, 48, 0.3)',
              borderRadius: '8px',
              padding: '16px',
              fontSize: '14px',
              color: '#FF3B30',
              marginBottom: '16px'
            }}>
              {error}
            </div>
          )}

          {!wallet.isConnected && (
            <div style={{ 
              background: 'rgba(0, 255, 136, 0.1)', 
              border: '1px solid rgba(0, 255, 136, 0.3)',
              borderRadius: '8px',
              padding: '16px',
              fontSize: '14px',
              color: '#00FF88'
            }}>
              <strong>Connect MetaMask</strong><br />
              Connect your wallet to start trading on Polymarket.
            </div>
          )}

          {wallet.isConnected && wallet.chainId !== 137 && (
            <div style={{ 
              background: 'rgba(255, 178, 36, 0.1)', 
              border: '1px solid rgba(255, 178, 36, 0.3)',
              borderRadius: '8px',
              padding: '16px',
              fontSize: '14px',
              color: '#FFB224'
            }}>
              <strong>Wrong Network</strong><br />
              Please switch to Polygon network to trade.
            </div>
          )}
        </Sidebar>
      </TradeContainer>
    </>
  );
}
