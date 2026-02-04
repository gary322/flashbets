import React, { useState, useMemo } from 'react';
import styled from '@emotion/styled';
import { motion } from 'framer-motion';
import { Market, PriceUpdate } from '../../types';
import { BlurCard } from '../core/BlurCard';

interface MarketSelectorProps {
  markets: Market[];
  selectedMarket: Market | null;
  onSelect: (market: Market) => void;
  prices: Map<string, PriceUpdate>;
  searchPlaceholder?: string;
}

const Container = styled.div`
  height: 100%;
  display: flex;
  flex-direction: column;
`;

const SearchInput = styled.input`
  background: ${props => props.theme.colors.background.primary};
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 12px 16px;
  color: ${props => props.theme.colors.text.primary};
  font-size: 14px;
  margin-bottom: 16px;
  transition: all 200ms ease;

  &:focus {
    outline: none;
    border-color: ${props => props.theme.colors.accent.primary};
  }

  &::placeholder {
    color: ${props => props.theme.colors.text.tertiary};
  }
`;

const MarketList = styled.div`
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
`;

const MarketItem = styled(motion.div)<{ selected: boolean }>`
  padding: 16px;
  cursor: pointer;
  background: ${props => props.selected ? 
    props.theme.colors.background.tertiary : 
    'transparent'};
  border: 1px solid ${props => props.selected ?
    props.theme.colors.accent.primary :
    'transparent'};
  border-radius: 8px;
  transition: all 200ms ease;

  &:hover {
    background: ${props => props.theme.colors.background.tertiary};
  }
`;

const MarketName = styled.div`
  font-size: 14px;
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
  margin-bottom: 4px;
`;

const MarketPrice = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
`;

const Price = styled.span`
  font-family: ${props => props.theme.typography.fonts.mono};
  font-size: 16px;
  font-weight: 700;
`;

const PriceChange = styled.span<{ positive: boolean }>`
  font-size: 12px;
  color: ${props => props.positive ? 
    props.theme.colors.accent.primary : 
    props.theme.colors.accent.secondary};
`;

export const MarketSelector: React.FC<MarketSelectorProps> = ({
  markets,
  selectedMarket,
  onSelect,
  prices,
  searchPlaceholder = "Search markets..."
}) => {
  const [search, setSearch] = useState('');

  const filteredMarkets = useMemo(() => {
    const searchLower = search.toLowerCase();
    return markets.filter(market => 
      market.name.toLowerCase().includes(searchLower)
    );
  }, [markets, search]);

  const formatPrice = (price: number) => {
    return `${(price * 100).toFixed(1)}%`;
  };

  const formatChange = (change: number) => {
    const sign = change >= 0 ? '+' : '';
    return `${sign}${change.toFixed(2)}%`;
  };

  return (
    <Container>
      <SearchInput
        type="text"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder={searchPlaceholder}
      />
      
      <MarketList>
        {filteredMarkets.map(market => {
          const priceData = prices.get(market.id);
          const currentPrice = priceData?.price || market.lastPrice;
          const change = priceData?.changePercent || market.change24h;
          
          return (
            <MarketItem
              key={market.id}
              selected={selectedMarket?.id === market.id}
              onClick={() => onSelect(market)}
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
            >
              <MarketName>{market.name}</MarketName>
              <MarketPrice>
                <Price>{formatPrice(currentPrice)}</Price>
                <PriceChange positive={change >= 0}>
                  {formatChange(change)}
                </PriceChange>
              </MarketPrice>
            </MarketItem>
          );
        })}
      </MarketList>
    </Container>
  );
};