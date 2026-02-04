import React from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';

const LeaderboardContainer = styled.div`
  max-width: 1440px;
  margin: 0 auto;
  padding: 32px 24px;
`;

const Header = styled.div`
  margin-bottom: 48px;
  text-align: center;
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

const TimeFilter = styled.div`
  display: flex;
  justify-content: center;
  gap: 16px;
  margin-bottom: 48px;
`;

const FilterButton = styled.button<{ active?: boolean }>`
  padding: 12px 24px;
  border-radius: 8px;
  border: 1px solid ${props => props.active ? props.theme.colors.accent.primary : 'rgba(255, 255, 255, 0.1)'};
  background: ${props => props.active ? props.theme.colors.accent.primary : 'transparent'};
  color: ${props => props.active ? '#000' : props.theme.colors.text.primary};
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    border-color: ${props => props.theme.colors.accent.primary};
    background: ${props => !props.active && 'rgba(0, 255, 136, 0.1)'};
  }
`;

const LeaderboardTable = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  overflow: hidden;
`;

const TableHeader = styled.div`
  display: grid;
  grid-template-columns: 80px 1fr 1fr 1fr 1fr;
  padding: 16px 24px;
  background: rgba(255, 255, 255, 0.02);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const TableRow = styled.div`
  display: grid;
  grid-template-columns: 80px 1fr 1fr 1fr 1fr;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  transition: background 0.2s ease;
  align-items: center;
  
  &:hover {
    background: rgba(255, 255, 255, 0.02);
  }
  
  &:last-child {
    border-bottom: none;
  }
`;

const Rank = styled.div<{ rank: number }>`
  font-size: 20px;
  font-weight: 800;
  color: ${props => {
    if (props.rank === 1) return '#FFD700';
    if (props.rank === 2) return '#C0C0C0';
    if (props.rank === 3) return '#CD7F32';
    return props.theme.colors.text.primary;
  }};
`;

const UserInfo = styled.div`
  display: flex;
  align-items: center;
  gap: 12px;
`;

const Avatar = styled.div`
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 600;
  color: white;
`;

const Username = styled.div`
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
`;

const PnL = styled.div<{ positive?: boolean }>`
  font-weight: 600;
  font-size: 18px;
  color: ${props => props.positive ? '#00FF88' : '#FF4444'};
`;

const Stat = styled.div`
  color: ${props => props.theme.colors.text.primary};
  font-weight: 500;
`;

const TopTraders = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 24px;
  margin-bottom: 48px;
`;

const TopTraderCard = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
  text-align: center;
  position: relative;
  overflow: hidden;
  
  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: ${props => props.theme.colors.accent.primary};
  }
`;

const Medal = styled.div`
  font-size: 48px;
  margin-bottom: 16px;
`;

const TraderName = styled.h3`
  font-size: 20px;
  font-weight: 700;
  margin-bottom: 8px;
  color: ${props => props.theme.colors.text.primary};
`;

const TraderStats = styled.div`
  font-size: 24px;
  font-weight: 700;
  color: #00FF88;
  margin-bottom: 8px;
`;

const TraderSubstat = styled.div`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
`;

export default function Leaderboard() {
  const [timeframe, setTimeframe] = React.useState<'daily' | 'weekly' | 'monthly' | 'all'>('weekly');

  const topTraders = [
    { rank: 1, name: 'DeFiWhale', pnl: '+$125,430', winRate: '78%' },
    { rank: 2, name: 'CryptoSage', pnl: '+$98,220', winRate: '72%' },
    { rank: 3, name: 'MoonTrader', pnl: '+$87,650', winRate: '69%' },
  ];

  const leaderboardData = [
    { rank: 4, name: 'SolanaMaxi', pnl: '+$65,320', trades: 234, winRate: '65%', volume: '$1.2M' },
    { rank: 5, name: 'PredictPro', pnl: '+$54,890', trades: 189, winRate: '61%', volume: '$980K' },
    { rank: 6, name: 'MarketMaker', pnl: '+$48,750', trades: 412, winRate: '58%', volume: '$2.1M' },
    { rank: 7, name: 'QubitTrader', pnl: '+$41,200', trades: 156, winRate: '64%', volume: '$750K' },
    { rank: 8, name: 'AlphaSeeker', pnl: '+$38,900', trades: 298, winRate: '55%', volume: '$1.5M' },
  ];

  return (
    <>
      <Head>
        <title>Leaderboard - Betting Platform</title>
        <meta name="description" content="Top traders on the platform" />
      </Head>

      <LeaderboardContainer>
        <Header>
          <Title>Leaderboard</Title>
          <Subtitle>The best traders on the platform</Subtitle>
        </Header>

        <TimeFilter>
          <FilterButton 
            active={timeframe === 'daily'} 
            onClick={() => setTimeframe('daily')}
          >
            24H
          </FilterButton>
          <FilterButton 
            active={timeframe === 'weekly'} 
            onClick={() => setTimeframe('weekly')}
          >
            7D
          </FilterButton>
          <FilterButton 
            active={timeframe === 'monthly'} 
            onClick={() => setTimeframe('monthly')}
          >
            30D
          </FilterButton>
          <FilterButton 
            active={timeframe === 'all'} 
            onClick={() => setTimeframe('all')}
          >
            All Time
          </FilterButton>
        </TimeFilter>

        <TopTraders>
          {topTraders.map((trader) => (
            <TopTraderCard key={trader.rank}>
              <Medal>{trader.rank === 1 ? '🥇' : trader.rank === 2 ? '🥈' : '🥉'}</Medal>
              <TraderName>{trader.name}</TraderName>
              <TraderStats>{trader.pnl}</TraderStats>
              <TraderSubstat>{trader.winRate} Win Rate</TraderSubstat>
            </TopTraderCard>
          ))}
        </TopTraders>

        <LeaderboardTable>
          <TableHeader>
            <div>Rank</div>
            <div>Trader</div>
            <div>P&L</div>
            <div>Win Rate</div>
            <div>Volume</div>
          </TableHeader>

          {leaderboardData.map((trader) => (
            <TableRow key={trader.rank}>
              <Rank rank={trader.rank}>#{trader.rank}</Rank>
              <UserInfo>
                <Avatar>{trader.name.charAt(0)}</Avatar>
                <Username>{trader.name}</Username>
              </UserInfo>
              <PnL positive>{trader.pnl}</PnL>
              <Stat>{trader.winRate}</Stat>
              <Stat>{trader.volume}</Stat>
            </TableRow>
          ))}
        </LeaderboardTable>
      </LeaderboardContainer>
    </>
  );
}