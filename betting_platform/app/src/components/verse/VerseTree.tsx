import React from 'react';
import styled from '@emotion/styled';

export interface VerseNode {
  id: string;
  name: string;
  icon?: string;
  type: 'root' | 'category' | 'subcategory' | 'market';
  children?: VerseNode[];
  marketCount?: number;
  multiplier?: number;
  active?: boolean;
}

interface VerseTreeProps {
  verses: VerseNode[];
  selectedVerseId?: string;
  onVerseSelect?: (verse: VerseNode) => void;
  expandedVerses?: Set<string>;
  onToggleExpand?: (verseId: string) => void;
}

const TreeContainer = styled.div`
  font-size: 14px;
`;

const NodeContainer = styled.div<{ depth: number }>`
  margin-left: ${props => props.depth * 28}px;
  margin-bottom: 4px;
`;

const NodeItem = styled.div<{ selected?: boolean; active?: boolean }>`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: ${props => props.theme?.components?.verseTree?.borderRadius || '6px'};
  cursor: pointer;
  transition: all ${props => props.theme?.animation?.durations?.fast || '150ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  background: ${props => {
    if (props.selected) return 'rgba(255, 214, 10, 0.1)';
    if (props.active) return 'rgba(255, 255, 255, 0.03)';
    return 'transparent';
  }};
  border: 1px solid ${props => {
    if (props.selected) return 'rgba(255, 214, 10, 0.3)';
    return 'transparent';
  }};
  
  &:hover {
    background: rgba(255, 255, 255, 0.05);
    ${props => !props.selected && `
      border-color: rgba(255, 255, 255, 0.1);
    `}
  }
`;

const ExpandIcon = styled.div<{ expanded?: boolean }>`
  width: ${props => props.theme?.components?.verseTree?.expandIconSize || '16px'};
  height: ${props => props.theme?.components?.verseTree?.expandIconSize || '16px'};
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform ${props => props.theme?.animation?.durations?.fast || '150ms'} ${props => props.theme?.animation?.easings?.default || 'ease'};
  transform: ${props => props.expanded ? 'rotate(90deg)' : 'rotate(0)'};
  opacity: 0.5;
`;

const VerseIcon = styled.div<{ type: string }>`
  width: ${props => props.theme?.components?.verseTree?.iconSize || '24px'};
  height: ${props => props.theme?.components?.verseTree?.iconSize || '24px'};
  border-radius: 4px;
  background: ${props => {
    const colors = props.theme?.colors?.verse;
    if (!colors) return 'rgba(255, 255, 255, 0.1)';
    switch (props.type) {
      case 'politics': return colors.politics || '#ff6b6b';
      case 'crypto': return colors.crypto || '#4ecdc4';
      case 'sports': return colors.sports || '#45b7d1';
      case 'science': return colors.science || '#96ceb4';
      default: return 'rgba(255, 255, 255, 0.1)';
    }
  }};
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
`;

const VerseLabel = styled.span`
  flex: 1;
  color: ${props => props.theme?.colors?.text?.primary || '#fff'};
`;

const VerseInfo = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
`;

const VerseCount = styled.span`
  font-size: 12px;
  color: ${props => props.theme?.colors?.text?.tertiary || '#666'};
`;

const VerseMultiplier = styled.span`
  font-size: 12px;
  font-weight: 600;
  color: ${props => props.theme?.colors?.accent?.secondary || '#ffa500'};
  padding: 2px 6px;
  background: rgba(255, 165, 0, 0.1);
  border-radius: 4px;
`;

const ChildrenContainer = styled.div<{ expanded: boolean }>`
  display: ${props => props.expanded ? 'block' : 'none'};
  margin-top: 4px;
`;

const getVerseIcon = (verse: VerseNode): string => {
  if (verse.icon) return verse.icon;
  
  const iconMap: Record<string, string> = {
    'politics': '🏛️',
    'crypto': '₿',
    'sports': '⚽',
    'science': '🔬',
    'tech': '💻',
    'entertainment': '🎬',
    'economics': '📈',
    'us-politics': '🇺🇸',
    'world-politics': '🌍',
    'bitcoin': '₿',
    'ethereum': 'Ξ',
    'defi': '🏦',
    'ai': '🤖',
    'nfl': '🏈',
    'nba': '🏀',
  };

  const key = verse.id.split('-').find(part => iconMap[part]);
  return iconMap[key || ''] || '📊';
};

const getVerseType = (verseId: string): string => {
  if (verseId.includes('politics')) return 'politics';
  if (verseId.includes('crypto') || verseId.includes('bitcoin')) return 'crypto';
  if (verseId.includes('sports') || verseId.includes('nfl') || verseId.includes('nba')) return 'sports';
  if (verseId.includes('science') || verseId.includes('tech') || verseId.includes('ai')) return 'science';
  return 'default';
};

const VerseTreeNode: React.FC<{
  verse: VerseNode;
  depth: number;
  selected?: boolean;
  expanded?: boolean;
  onSelect?: (verse: VerseNode) => void;
  onToggleExpand?: () => void;
}> = ({ verse, depth, selected, expanded, onSelect, onToggleExpand }) => {
  const hasChildren = verse.children && verse.children.length > 0;
  
  const handleClick = (e: React.MouseEvent) => {
    if (hasChildren && e.target === e.currentTarget) {
      onToggleExpand?.();
    }
    onSelect?.(verse);
  };

  const handleExpandClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onToggleExpand?.();
  };

  return (
    <>
      <NodeContainer depth={depth}>
        <NodeItem 
          selected={selected} 
          active={verse.active}
          onClick={handleClick}
        >
          {hasChildren && (
            <ExpandIcon 
              expanded={expanded} 
              onClick={handleExpandClick}
            >
              ▶
            </ExpandIcon>
          )}
          {!hasChildren && <div style={{ width: '16px' }} />}
          
          <VerseIcon type={getVerseType(verse.id)}>
            {getVerseIcon(verse)}
          </VerseIcon>
          
          <VerseLabel>{verse.name}</VerseLabel>
          
          <VerseInfo>
            {verse.marketCount !== undefined && verse.marketCount > 0 && (
              <VerseCount>{verse.marketCount}</VerseCount>
            )}
            {verse.multiplier && verse.multiplier > 1 && (
              <VerseMultiplier>{verse.multiplier}x</VerseMultiplier>
            )}
          </VerseInfo>
        </NodeItem>
      </NodeContainer>
      
      {hasChildren && (
        <ChildrenContainer expanded={expanded || false}>
          {verse.children!.map(child => (
            <VerseTreeNode
              key={child.id}
              verse={child}
              depth={depth + 1}
              selected={selected}
              expanded={expanded}
              onSelect={onSelect}
              onToggleExpand={onToggleExpand}
            />
          ))}
        </ChildrenContainer>
      )}
    </>
  );
};

export default function VerseTree({
  verses,
  selectedVerseId,
  onVerseSelect,
  expandedVerses = new Set(),
  onToggleExpand
}: VerseTreeProps) {
  return (
    <TreeContainer>
      {verses.map(verse => (
        <VerseTreeNode
          key={verse.id}
          verse={verse}
          depth={0}
          selected={selectedVerseId === verse.id}
          expanded={expandedVerses.has(verse.id)}
          onSelect={onVerseSelect}
          onToggleExpand={() => onToggleExpand?.(verse.id)}
        />
      ))}
    </TreeContainer>
  );
}