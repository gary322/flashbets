import React, { useState } from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';
import { useRouter } from 'next/router';
import ThreePanelLayout from '../components/layout/ThreePanelLayout';
import LeftPanel from '../components/layout/LeftPanel';
import RightPanel from '../components/layout/RightPanel';
import VerseTree, { VerseNode } from '../components/verse/VerseTree';
import VerseCard from '../components/verse/VerseCard';
import QuantumToggle from '../components/quantum/QuantumToggle';
import QuantumStateDisplay from '../components/quantum/QuantumStateDisplay';
import { useVerses } from '../hooks/useVerses';
import { useQuantumContext } from '../contexts/QuantumContext';
import { useVerseContext } from '../contexts/VerseContext';
import { VerseProvider } from '../contexts/VerseContext';
import { QuantumProvider } from '../contexts/QuantumContext';

const MainContent = styled.div`
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
`;

const MarketHeader = styled.div`
  padding: 32px;
  border-bottom: 1px solid ${props => props.theme?.colors?.text?.tertiary || '#666'};
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.02) 0%, transparent 100%);
`;

const HeaderContent = styled.div`
  max-width: 1200px;
  margin: 0 auto;
`;

const MarketTitle = styled.h1`
  font-size: 28px;
  font-weight: 600;
  margin-bottom: 12px;
  color: ${props => props.theme?.colors?.text?.primary || '#fff'};
`;

const MarketMeta = styled.div`
  display: flex;
  gap: 24px;
  flex-wrap: wrap;
`;

const MetaItem = styled.div`
  display: flex;
  flex-direction: column;
  gap: 4px;
`;

const MetaLabel = styled.span`
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${props => props.theme?.colors?.text?.tertiary || '#666'};
`;

const MetaValue = styled.span`
  font-size: 16px;
  font-weight: 500;
  color: ${props => props.theme?.colors?.text?.primary || '#fff'};
`;

const MarketContent = styled.div`
  flex: 1;
  overflow-y: auto;
  padding: 32px;
`;

const SectionTitle = styled.h2`
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${props => props.theme?.colors?.text?.tertiary || '#666'};
  margin-bottom: 16px;
`;

const OutcomeGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
  margin-bottom: 32px;
`;

const OutcomeCard = styled.div<{ selected?: boolean }>`
  background: ${props => props.theme?.colors?.background?.tertiary || 'rgba(255, 255, 255, 0.05)'};
  border: 1px solid ${props => props.selected 
    ? props.theme?.colors?.accent?.primary || '#ffa500'
    : 'rgba(255, 255, 255, 0.1)'
  };
  border-radius: 12px;
  padding: 20px;
  cursor: pointer;
  transition: all ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  
  &:hover {
    background: rgba(255, 255, 255, 0.05);
    transform: translateY(-2px);
  }
`;

const OutcomeName = styled.div`
  font-size: 16px;
  font-weight: 500;
  margin-bottom: 8px;
`;

const OutcomePrice = styled.div`
  font-size: 32px;
  font-weight: 300;
  margin-bottom: 8px;
  color: ${props => props.theme?.colors?.accent?.primary || '#ffa500'};
`;

const OutcomeChange = styled.div<{ positive?: boolean }>`
  font-size: 14px;
  color: ${props => props.positive 
    ? props.theme?.colors?.status?.success || '#00ff00'
    : props.theme?.colors?.status?.error || '#ff0000'
  };
`;

const VerseGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
`;

const MarketsList = styled.div`
  display: grid;
  gap: 16px;
`;

const MarketCard = styled.div`
  background: ${props => props.theme?.colors?.background?.tertiary || 'rgba(255, 255, 255, 0.05)'};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
  cursor: pointer;
  transition: all ${props => props.theme?.animation?.durations?.normal || '300ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  
  &:hover {
    border-color: ${props => props.theme?.colors?.accent?.primary || '#ffa500'};
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
`;

const MarketCardTitle = styled.h3`
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 8px;
`;

const MarketCardDescription = styled.p`
  font-size: 14px;
  color: ${props => props.theme?.colors?.text?.secondary || '#aaa'};
  margin-bottom: 16px;
`;

function MarketsQuantumContent() {
  const router = useRouter();
  const [selectedMarket, setSelectedMarket] = useState<any>(null);
  const [selectedOutcome, setSelectedOutcome] = useState(0);
  const [markets, setMarkets] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  
  const { verses } = useVerses();
  const verseContext = useVerseContext();
  const quantumContext = useQuantumContext();

  React.useEffect(() => {
    fetchMarkets();
  }, []);

  const fetchMarkets = async () => {
    try {
      const response = await fetch('/api/markets?limit=20');
      const data = await response.json();
      setMarkets(data.markets || []);
      if (data.markets && data.markets.length > 0) {
        setSelectedMarket(data.markets[0]);
      }
    } catch (error) {
      console.error('Failed to fetch markets:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleMarketClick = (market: any) => {
    setSelectedMarket(market);
  };

  const handleExecuteTrade = async () => {
    if (!selectedMarket) return;
    
    // Navigate to trade page with market
    router.push(`/trade?market=${selectedMarket.id}`);
  };

  // Sample verse tree data
  const sampleVerseTree: VerseNode[] = [
    {
      id: 'politics-root',
      name: 'Politics & Elections',
      type: 'root',
      icon: '🏛️',
      children: [
        {
          id: 'us-politics',
          name: 'US Politics',
          type: 'category',
          multiplier: 1.5,
          children: [
            {
              id: 'presidential-2024',
              name: '2024 Presidential',
              type: 'subcategory',
              multiplier: 2,
              marketCount: 15,
            },
          ],
        },
      ],
    },
    {
      id: 'crypto-root',
      name: 'Cryptocurrency',
      type: 'root',
      icon: '₿',
      children: [
        {
          id: 'bitcoin-verse',
          name: 'Bitcoin Markets',
          type: 'category',
          multiplier: 2,
          marketCount: 8,
        },
      ],
    },
  ];

  // Calculate quantum states
  const quantumStates = quantumContext.isQuantumEnabled && selectedMarket
    ? quantumContext.calculateQuantumStates(selectedMarket.outcomes || [], 100)
    : [];

  return (
    <ThreePanelLayout
      leftPanel={
        <LeftPanel
          verseTree={
            <VerseTree
              verses={sampleVerseTree}
              selectedVerseId={undefined}
              onVerseSelect={(verse) => verseContext.selectVerse(verse.id)}
              expandedVerses={verseContext.expandedVerseIds}
              onToggleExpand={verseContext.toggleVerseExpansion}
            />
          }
        />
      }
      mainContent={
        <MainContent>
          {selectedMarket ? (
            <>
              <MarketHeader>
                <HeaderContent>
                  <MarketTitle>{selectedMarket.title}</MarketTitle>
                  <MarketMeta>
                    <MetaItem>
                      <MetaLabel>Volume 24h</MetaLabel>
                      <MetaValue>${(selectedMarket.total_volume / 1000).toFixed(0)}K</MetaValue>
                    </MetaItem>
                    <MetaItem>
                      <MetaLabel>Liquidity</MetaLabel>
                      <MetaValue>${(selectedMarket.total_liquidity / 1000).toFixed(0)}K</MetaValue>
                    </MetaItem>
                    <MetaItem>
                      <MetaLabel>Ends</MetaLabel>
                      <MetaValue>
                        {new Date(selectedMarket.resolution_time * 1000).toLocaleDateString()}
                      </MetaValue>
                    </MetaItem>
                  </MarketMeta>
                </HeaderContent>
              </MarketHeader>
              
              <MarketContent>
                <SectionTitle>Market Outcomes</SectionTitle>
                <OutcomeGrid>
                  {selectedMarket.outcomes?.map((outcome: any, index: number) => (
                    <OutcomeCard
                      key={index}
                      selected={selectedOutcome === index}
                      onClick={() => setSelectedOutcome(index)}
                    >
                      <OutcomeName>{outcome.name}</OutcomeName>
                      <OutcomePrice>{(outcome.price || 0.5).toFixed(2)}</OutcomePrice>
                      <OutcomeChange positive={outcome.change > 0}>
                        {outcome.change > 0 ? '+' : ''}{(outcome.change || 0).toFixed(2)}%
                      </OutcomeChange>
                    </OutcomeCard>
                  ))}
                </OutcomeGrid>

                <SectionTitle>Related Verses - Multiply Your Leverage</SectionTitle>
                <VerseGrid>
                  {verses.slice(0, 3).map((verse) => (
                    <VerseCard
                      key={verse.id}
                      verse={verse}
                      selected={verseContext.selectedVerseIds.has(verse.id)}
                      onClick={() => verseContext.selectVerse(verse.id)}
                    />
                  ))}
                </VerseGrid>
              </MarketContent>
            </>
          ) : (
            <MarketContent>
              <SectionTitle>All Markets</SectionTitle>
              <MarketsList>
                {markets.map((market) => (
                  <MarketCard 
                    key={market.id}
                    onClick={() => handleMarketClick(market)}
                  >
                    <MarketCardTitle>{market.title}</MarketCardTitle>
                    <MarketCardDescription>{market.description}</MarketCardDescription>
                  </MarketCard>
                ))}
              </MarketsList>
            </MarketContent>
          )}
        </MainContent>
      }
      rightPanel={
        <RightPanel
          selectedMarket={selectedMarket}
          selectedOutcome={selectedOutcome}
          onOutcomeSelect={setSelectedOutcome}
          quantumToggle={
            <QuantumToggle
              active={quantumContext.isQuantumEnabled}
              onChange={quantumContext.setQuantumEnabled}
            />
          }
          quantumStates={
            <QuantumStateDisplay
              states={quantumStates}
              totalAmount={100}
              isActive={quantumContext.isQuantumEnabled}
            />
          }
          onExecuteTrade={handleExecuteTrade}
        />
      }
    />
  );
}

export default function MarketsQuantum() {
  return (
    <>
      <Head>
        <title>Quantum Markets - Betting Platform</title>
        <meta name="description" content="Trade prediction markets with quantum superposition" />
      </Head>
      
      <VerseProvider>
        <QuantumProvider>
          <MarketsQuantumContent />
        </QuantumProvider>
      </VerseProvider>
    </>
  );
}
