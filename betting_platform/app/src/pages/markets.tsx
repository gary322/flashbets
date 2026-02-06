import React, { useEffect, useState } from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';
import { useRouter } from 'next/router';

const MarketsContainer = styled.div`
  max-width: 1440px;
  margin: 0 auto;
  padding: 32px 24px;
`;

const MarketsHeader = styled.div`
  margin-bottom: 32px;
`;

const Title = styled.h1`
  font-size: 48px;
  font-weight: 900;
  margin-bottom: 16px;
  color: ${props => props.theme.colors.text.primary};
`;

const Subtitle = styled.p`
  font-size: 20px;
  color: ${props => props.theme.colors.text.secondary};
`;

const SearchAndFilters = styled.div`
  display: flex;
  gap: 16px;
  margin-bottom: 32px;
  flex-wrap: wrap;
`;

const SearchInput = styled.input`
  flex: 1;
  min-width: 300px;
  padding: 12px 16px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: ${props => props.theme.colors.background.secondary};
  color: ${props => props.theme.colors.text.primary};
  font-size: 16px;
  
  &::placeholder {
    color: ${props => props.theme.colors.text.secondary};
  }
  
  &:focus {
    outline: none;
    border-color: ${props => props.theme.colors.accent.primary};
  }
`;

const FilterButton = styled.button<{ active?: boolean }>`
  padding: 12px 24px;
  border-radius: 8px;
  border: 1px solid ${props => props.active ? props.theme.colors.accent.primary : 'rgba(255, 255, 255, 0.1)'};
  background: ${props => props.active ? props.theme.colors.accent.primary : 'transparent'};
  color: ${props => props.active ? '#000' : props.theme.colors.text.primary};
  font-size: 16px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
    background: ${props => !props.active && 'rgba(0, 255, 136, 0.1)'};
  }
`;

const MarketsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
  gap: 24px;
`;

const MarketCard = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
`;

const MarketTitle = styled.h3`
  font-size: 20px;
  font-weight: 700;
  margin-bottom: 8px;
  color: ${props => props.theme.colors.text.primary};
`;

const MarketDescription = styled.p`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 16px;
  line-height: 1.5;
`;

const MarketStats = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  gap: 16px;
`;

const Stat = styled.div`
  text-align: center;
`;

const StatLabel = styled.div`
  font-size: 12px;
  color: ${props => props.theme.colors.text.secondary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const StatValue = styled.div`
  font-size: 18px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
`;

const OutcomesContainer = styled.div`
  display: flex;
  gap: 8px;
`;

const OutcomeButton = styled.button`
  flex: 1;
  padding: 12px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: transparent;
  color: ${props => props.theme.colors.text.primary};
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    background: rgba(0, 255, 136, 0.1);
    border-color: ${props => props.theme.colors.accent.primary};
  }
`;

const LoadingContainer = styled.div`
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 400px;
  font-size: 20px;
  color: ${props => props.theme.colors.text.secondary};
`;

const ErrorContainer = styled.div`
  background: rgba(255, 0, 0, 0.1);
  border: 1px solid rgba(255, 0, 0, 0.3);
  border-radius: 8px;
  padding: 24px;
  margin: 32px 0;
  color: #ff6b6b;
  text-align: center;
`;

interface Market {
  id: number;
  title: string;
  description: string;
  outcomes: Array<{ name: string; total_stake: number }>;
  total_liquidity: number;
  total_volume: number;
  resolution_time: number;
  resolved: boolean;
}

export default function Markets() {
  const router = useRouter();
  const [markets, setMarkets] = useState<Market[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState('');
  const [filter, setFilter] = useState<'all' | 'active' | 'resolved'>('all');

  useEffect(() => {
    const fetchMarkets = async () => {
      try {
        setLoading(true);
        setError(null);

        let url = '/api/markets?limit=50';
        if (filter === 'active') {
          url += '&status=active';
        } else if (filter === 'resolved') {
          url += '&status=resolved';
        }

        const response = await fetch(url);
        if (!response.ok) {
          throw new Error(`Failed to fetch markets: ${response.statusText}`);
        }

        const data = await response.json();
        setMarkets(data.markets || []);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load markets');
      } finally {
        setLoading(false);
      }
    };

    fetchMarkets();
  }, [filter]);

  const filteredMarkets = markets.filter(market => 
    market.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
    market.description.toLowerCase().includes(searchTerm.toLowerCase())
  );

  const formatVolume = (volume: number) => {
    if (volume >= 1000000) {
      return `$${(volume / 1000000).toFixed(1)}M`;
    } else if (volume >= 1000) {
      return `$${(volume / 1000).toFixed(0)}K`;
    }
    return `$${volume}`;
  };

  const formatDate = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleDateString('en-US', { 
      month: 'short', 
      day: 'numeric',
      year: 'numeric'
    });
  };

  const handleMarketClick = (marketId: number) => {
    router.push(`/trade?market=${marketId}`);
  };

  return (
    <>
      <Head>
        <title>Markets - Betting Platform</title>
        <meta name="description" content="Browse and trade prediction markets" />
      </Head>

      <MarketsContainer>
        <MarketsHeader>
          <Title>Prediction Markets</Title>
          <Subtitle>Trade on real-world events with up to 500x leverage</Subtitle>
        </MarketsHeader>

        <SearchAndFilters>
          <SearchInput
            type="text"
            placeholder="Search markets..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
          />
          <FilterButton 
            active={filter === 'all'} 
            onClick={() => setFilter('all')}
          >
            All Markets
          </FilterButton>
          <FilterButton 
            active={filter === 'active'} 
            onClick={() => setFilter('active')}
          >
            Active
          </FilterButton>
          <FilterButton 
            active={filter === 'resolved'} 
            onClick={() => setFilter('resolved')}
          >
            Resolved
          </FilterButton>
        </SearchAndFilters>

        {loading && (
          <LoadingContainer>Loading markets...</LoadingContainer>
        )}

        {error && (
          <ErrorContainer>{error}</ErrorContainer>
        )}

        {!loading && !error && (
          <MarketsGrid>
            {filteredMarkets.map((market) => (
              <MarketCard 
                key={market.id} 
                onClick={() => handleMarketClick(market.id)}
                data-testid="market-card"
                data-market-id={market.id}
              >
                <MarketTitle>{market.title}</MarketTitle>
                <MarketDescription>{market.description}</MarketDescription>
                
                <MarketStats>
                  <Stat>
                    <StatLabel>Volume</StatLabel>
                    <StatValue>{formatVolume(market.total_volume)}</StatValue>
                  </Stat>
                  <Stat>
                    <StatLabel>Liquidity</StatLabel>
                    <StatValue>{formatVolume(market.total_liquidity)}</StatValue>
                  </Stat>
                  <Stat>
                    <StatLabel>Ends</StatLabel>
                    <StatValue>{formatDate(market.resolution_time)}</StatValue>
                  </Stat>
                </MarketStats>

                <OutcomesContainer>
                  {market.outcomes.map((outcome, index) => (
                    <OutcomeButton key={index}>
                      {outcome.name}
                    </OutcomeButton>
                  ))}
                </OutcomesContainer>
              </MarketCard>
            ))}
          </MarketsGrid>
        )}

        {!loading && !error && filteredMarkets.length === 0 && (
          <LoadingContainer>No markets found</LoadingContainer>
        )}
      </MarketsContainer>
    </>
  );
}
