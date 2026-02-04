import React from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';
import Link from 'next/link';
import { useRouter } from 'next/router';

const HomeContainer = styled.div`
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  background: ${props => props.theme.colors.background.primary};
  padding: 24px;
`;

const HeroSection = styled.div`
  text-align: center;
  max-width: 800px;
  margin-bottom: 64px;
`;

const Title = styled.h1`
  font-size: 64px;
  font-weight: 900;
  margin-bottom: 24px;
  background: linear-gradient(135deg, ${props => props.theme.colors.accent.primary} 0%, ${props => props.theme.colors.accent.secondary} 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  
  @media (max-width: ${props => props.theme.breakpoints.mobile}) {
    font-size: 48px;
  }
`;

const Subtitle = styled.p`
  font-size: 24px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 16px;
  font-weight: 300;
`;

const Description = styled.p`
  font-size: 18px;
  color: ${props => props.theme.colors.text.tertiary};
  line-height: 1.6;
`;

const ButtonGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 24px;
  max-width: 1200px;
  width: 100%;
`;

const FeatureCard = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 16px;
  padding: 32px;
  text-align: center;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.normal} ${props => props.theme.animation.easings.default};
  
  &:hover {
    background: ${props => props.theme.colors.background.tertiary};
    border-color: ${props => props.theme.colors.accent.primary};
    transform: translateY(-4px);
    box-shadow: 0 8px 32px rgba(255, 214, 10, 0.1);
  }
`;

const FeatureIcon = styled.div`
  font-size: 48px;
  margin-bottom: 16px;
`;

const FeatureTitle = styled.h3`
  font-size: 24px;
  font-weight: 600;
  margin-bottom: 12px;
  color: ${props => props.theme.colors.text.primary};
`;

const FeatureDescription = styled.p`
  font-size: 16px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 24px;
`;

const FeatureButton = styled.button`
  padding: 12px 24px;
  background: ${props => props.theme.colors.accent.primary};
  border: none;
  border-radius: 8px;
  color: ${props => props.theme.colors.text.inverse};
  font-weight: 600;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(255, 214, 10, 0.3);
  }
`;

const InfoSection = styled.div`
  display: flex;
  gap: 32px;
  margin-top: 64px;
  padding: 32px;
  background: rgba(255, 255, 255, 0.02);
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.05);
  
  @media (max-width: ${props => props.theme.breakpoints.tablet}) {
    flex-direction: column;
  }
`;

const InfoItem = styled.div`
  flex: 1;
  text-align: center;
`;

const InfoValue = styled.div`
  font-size: 48px;
  font-weight: 800;
  color: ${props => props.theme.colors.accent.primary};
  margin-bottom: 8px;
`;

const InfoLabel = styled.div`
  font-size: 16px;
  color: ${props => props.theme.colors.text.secondary};
`;

export default function Home() {
  const router = useRouter();

  const features = [
    {
      icon: '📊',
      title: 'Classic Markets',
      description: 'Trade prediction markets with leverage up to 100x',
      href: '/markets',
    },
    {
      icon: '⚛️',
      title: 'Quantum Markets',
      description: 'Experience quantum superposition trading with verse multipliers',
      href: '/markets-quantum',
    },
    {
      icon: '🎮',
      title: 'Demo Mode',
      description: 'Start with 10,000 SOL demo funds to test strategies',
      href: '/demo',
    },
  ];

  return (
    <>
      <Head>
        <title>Quantum Betting Platform - Native Solana</title>
        <meta name="description" content="Trade prediction markets with quantum superposition and verse multipliers" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <link rel="icon" href="/favicon.ico" />
      </Head>

      <HomeContainer>
        <HeroSection>
          <Title>Quantum Betting Platform</Title>
          <Subtitle>The Future of Prediction Markets on Solana</Subtitle>
          <Description>
            Experience the world's first quantum-enhanced prediction market platform. 
            Trade with superposition states, leverage verse multipliers, and unlock 
            unprecedented trading possibilities on native Solana.
          </Description>
        </HeroSection>

        <ButtonGrid>
          {features.map((feature) => (
            <FeatureCard 
              key={feature.href}
              onClick={() => router.push(feature.href)}
            >
              <FeatureIcon>{feature.icon}</FeatureIcon>
              <FeatureTitle>{feature.title}</FeatureTitle>
              <FeatureDescription>{feature.description}</FeatureDescription>
              <FeatureButton>Get Started</FeatureButton>
            </FeatureCard>
          ))}
        </ButtonGrid>

        <InfoSection>
          <InfoItem>
            <InfoValue>500x</InfoValue>
            <InfoLabel>Maximum Leverage</InfoLabel>
          </InfoItem>
          <InfoItem>
            <InfoValue>⚛️</InfoValue>
            <InfoLabel>Quantum Superposition</InfoLabel>
          </InfoItem>
          <InfoItem>
            <InfoValue>Native</InfoValue>
            <InfoLabel>Solana Integration</InfoLabel>
          </InfoItem>
        </InfoSection>
      </HomeContainer>
    </>
  );
}