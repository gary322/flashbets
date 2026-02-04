import React from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';

const PortfolioContainer = styled.div`
  max-width: 1440px;
  margin: 0 auto;
  padding: 32px 24px;
`;

const Header = styled.div`
  margin-bottom: 48px;
`;

const Title = styled.h1`
  font-size: 48px;
  font-weight: 900;
  margin-bottom: 16px;
  color: ${props => props.theme.colors.text.primary};
`;

const StatsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 24px;
  margin-bottom: 48px;
`;

const StatCard = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 24px;
`;

const StatLabel = styled.div`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
`;

const StatValue = styled.div`
  font-size: 32px;
  font-weight: 700;
  color: ${props => props.theme.colors.text.primary};
`;

const StatChange = styled.div<{ positive?: boolean }>`
  font-size: 14px;
  color: ${props => props.positive ? '#00FF88' : '#FF4444'};
  margin-top: 8px;
`;

const SectionTitle = styled.h2`
  font-size: 24px;
  font-weight: 700;
  margin-bottom: 24px;
  color: ${props => props.theme.colors.text.primary};
`;

const PositionsTable = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  overflow: hidden;
`;

const TableHeader = styled.div`
  display: grid;
  grid-template-columns: 2fr 1fr 1fr 1fr 1fr 1fr;
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
  grid-template-columns: 2fr 1fr 1fr 1fr 1fr 1fr;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  transition: background 0.2s ease;
  
  &:hover {
    background: rgba(255, 255, 255, 0.02);
  }
  
  &:last-child {
    border-bottom: none;
  }
`;

const MarketName = styled.div`
  font-weight: 600;
  color: ${props => props.theme.colors.text.primary};
`;

const EmptyState = styled.div`
  text-align: center;
  padding: 80px 24px;
  color: ${props => props.theme.colors.text.secondary};
`;

const EmptyStateTitle = styled.h3`
  font-size: 24px;
  font-weight: 700;
  margin-bottom: 16px;
  color: ${props => props.theme.colors.text.primary};
`;

const EmptyStateText = styled.p`
  font-size: 16px;
  margin-bottom: 24px;
  max-width: 400px;
  margin-left: auto;
  margin-right: auto;
`;

const CTAButton = styled.a`
  display: inline-block;
  padding: 16px 32px;
  border-radius: 8px;
  background: ${props => props.theme.colors.accent.primary};
  color: #000;
  font-weight: 600;
  text-decoration: none;
  transition: all 0.2s ease;
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
`;

const PnLValue = styled.div<{ positive?: boolean }>`
  font-weight: 600;
  color: ${props => props.positive ? '#00FF88' : '#FF4444'};
`;

export default function Portfolio() {
  const positions = []; // This would come from API

  return (
    <>
      <Head>
        <title>Portfolio - Betting Platform</title>
        <meta name="description" content="Manage your prediction market positions" />
      </Head>

      <PortfolioContainer>
        <Header>
          <Title>Portfolio</Title>
        </Header>

        <StatsGrid>
          <StatCard>
            <StatLabel>Total Balance</StatLabel>
            <StatValue>$0.00</StatValue>
            <StatChange positive>+0.00%</StatChange>
          </StatCard>
          
          <StatCard>
            <StatLabel>Total P&L</StatLabel>
            <StatValue>$0.00</StatValue>
            <StatChange positive>+0.00%</StatChange>
          </StatCard>
          
          <StatCard>
            <StatLabel>Open Positions</StatLabel>
            <StatValue>0</StatValue>
          </StatCard>
          
          <StatCard>
            <StatLabel>Total Volume</StatLabel>
            <StatValue>$0.00</StatValue>
          </StatCard>
        </StatsGrid>

        <div>
          <SectionTitle>Open Positions</SectionTitle>
          
          {positions.length > 0 ? (
            <PositionsTable>
              <TableHeader>
                <div>Market</div>
                <div>Side</div>
                <div>Size</div>
                <div>Entry</div>
                <div>Current</div>
                <div>P&L</div>
              </TableHeader>
              
              {/* Position rows would go here */}
            </PositionsTable>
          ) : (
            <PositionsTable>
              <EmptyState>
                <EmptyStateTitle>No Open Positions</EmptyStateTitle>
                <EmptyStateText>
                  Start trading to see your positions here. Track your P&L, manage risk, and monitor your portfolio performance.
                </EmptyStateText>
                <CTAButton href="/markets">
                  Explore Markets
                </CTAButton>
              </EmptyState>
            </PositionsTable>
          )}
        </div>
      </PortfolioContainer>
    </>
  );
}