import React from 'react';
import styled from '@emotion/styled';

export interface VerseData {
  id: string;
  name: string;
  description: string;
  multiplier: number;
  marketCount: number;
  totalVolume: number;
  participants: number;
  risk_tier: 'Low' | 'Medium' | 'High';
  category: string;
}

interface VerseCardProps {
  verse: VerseData;
  selected?: boolean;
  onClick?: () => void;
}

const Card = styled.div<{ selected?: boolean }>`
  background: ${props => props.theme.colors.background.tertiary};
  border: 1px solid ${props => props.selected 
    ? props.theme.colors.accent.secondary
    : 'rgba(255, 255, 255, 0.1)'
  };
  border-radius: 12px;
  padding: 20px;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.normal} ${props => props.theme.animation.easings.default};
  position: relative;
  
  &:hover {
    background: rgba(255, 255, 255, 0.05);
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
  
  ${props => props.selected && `
    background: rgba(255, 165, 0, 0.05);
    box-shadow: 0 0 0 2px rgba(255, 165, 0, 0.2);
  `}
`;

const VerseHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 12px;
`;

const VerseTitle = styled.h3`
  font-size: 16px;
  font-weight: 500;
  margin: 0;
  flex: 1;
  color: ${props => props.theme.colors.text.primary};
`;

const VerseMultiplier = styled.div`
  font-size: 24px;
  font-weight: 600;
  color: ${props => props.theme.colors.accent.secondary};
`;

const VerseDescription = styled.p`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 16px;
  line-height: 1.5;
`;

const VerseStats = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
`;

const VerseStat = styled.div`
  display: flex;
  flex-direction: column;
  gap: 2px;
`;

const StatLabel = styled.span`
  font-size: 11px;
  color: ${props => props.theme.colors.text.tertiary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const StatValue = styled.span`
  font-size: 14px;
  font-weight: 500;
  color: ${props => props.theme.colors.text.primary};
`;

const RiskTier = styled.div<{ tier: string }>`
  position: absolute;
  top: 12px;
  right: 12px;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 600;
  background: ${props => {
    switch (props.tier) {
      case 'Low': return 'rgba(76, 217, 100, 0.1)';
      case 'Medium': return 'rgba(255, 149, 0, 0.1)';
      case 'High': return 'rgba(255, 59, 48, 0.1)';
      default: return 'rgba(255, 255, 255, 0.1)';
    }
  }};
  color: ${props => {
    switch (props.tier) {
      case 'Low': return props.theme.colors.status.success;
      case 'Medium': return props.theme.colors.status.warning;
      case 'High': return props.theme.colors.status.error;
      default: return props.theme.colors.text.secondary;
    }
  }};
`;

const CategoryBadge = styled.div<{ category: string }>`
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  background: ${props => {
    const colors = props.theme.colors.verse;
    const cat = props.category.toLowerCase();
    if (cat.includes('politics')) return `${colors.politics}20`;
    if (cat.includes('crypto')) return `${colors.crypto}20`;
    if (cat.includes('sports')) return `${colors.sports}20`;
    if (cat.includes('science') || cat.includes('tech')) return `${colors.science}20`;
    return `${colors.default}20`;
  }};
  border-radius: 4px;
  font-size: 12px;
  color: ${props => {
    const colors = props.theme.colors.verse;
    const cat = props.category.toLowerCase();
    if (cat.includes('politics')) return colors.politics;
    if (cat.includes('crypto')) return colors.crypto;
    if (cat.includes('sports')) return colors.sports;
    if (cat.includes('science') || cat.includes('tech')) return colors.science;
    return colors.default;
  }};
  margin-bottom: 12px;
`;

const formatVolume = (volume: number): string => {
  if (volume >= 1000000) {
    return `$${(volume / 1000000).toFixed(1)}M`;
  } else if (volume >= 1000) {
    return `$${(volume / 1000).toFixed(0)}K`;
  }
  return `$${volume}`;
};

const formatNumber = (num: number): string => {
  if (num >= 1000) {
    return `${(num / 1000).toFixed(1)}K`;
  }
  return num.toString();
};

export default function VerseCard({ verse, selected, onClick }: VerseCardProps) {
  return (
    <Card selected={selected} onClick={onClick}>
      <RiskTier tier={verse.risk_tier}>
        {verse.risk_tier} Risk
      </RiskTier>
      
      <CategoryBadge category={verse.category}>
        {verse.category}
      </CategoryBadge>
      
      <VerseHeader>
        <VerseTitle>{verse.name}</VerseTitle>
        <VerseMultiplier>{verse.multiplier}x</VerseMultiplier>
      </VerseHeader>
      
      <VerseDescription>{verse.description}</VerseDescription>
      
      <VerseStats>
        <VerseStat>
          <StatLabel>Markets</StatLabel>
          <StatValue>{verse.marketCount}</StatValue>
        </VerseStat>
        <VerseStat>
          <StatLabel>Volume</StatLabel>
          <StatValue>{formatVolume(verse.totalVolume)}</StatValue>
        </VerseStat>
        <VerseStat>
          <StatLabel>Participants</StatLabel>
          <StatValue>{formatNumber(verse.participants)}</StatValue>
        </VerseStat>
        <VerseStat>
          <StatLabel>Leverage</StatLabel>
          <StatValue>Up to {verse.multiplier * 5}x</StatValue>
        </VerseStat>
      </VerseStats>
    </Card>
  );
}