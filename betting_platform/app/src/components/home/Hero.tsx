import React from 'react';
import styled from '@emotion/styled';
import Link from 'next/link';

const HeroSection = styled.section`
  min-height: 80vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 24px;
  background: linear-gradient(
    135deg,
    ${props => props.theme.colors.background.primary} 0%,
    ${props => props.theme.colors.background.secondary} 100%
  );
`;

const HeroContent = styled.div`
  max-width: 1200px;
  width: 100%;
  text-align: center;
`;

const HeroTitle = styled.h1`
  font-size: 64px;
  font-weight: 900;
  line-height: 1.2;
  margin-bottom: 24px;
  background: linear-gradient(
    135deg,
    ${props => props.theme.colors.text.primary} 0%,
    ${props => props.theme.colors.accent.primary} 100%
  );
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  
  @media (max-width: 768px) {
    font-size: 48px;
  }
`;

const HeroSubtitle = styled.p`
  font-size: 24px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 48px;
  max-width: 600px;
  margin-left: auto;
  margin-right: auto;
  
  @media (max-width: 768px) {
    font-size: 18px;
  }
`;

const CTAContainer = styled.div`
  display: flex;
  gap: 16px;
  justify-content: center;
  flex-wrap: wrap;
`;

const CTAButton = styled.span<{ primary?: boolean }>`
  display: inline-block;
  padding: 16px 32px;
  font-size: 18px;
  font-weight: 600;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  text-decoration: none;
  
  ${props => props.primary ? `
    background: ${props.theme.colors.accent.primary};
    color: #000;
    
    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    }
  ` : `
    background: transparent;
    color: ${props.theme.colors.text.primary};
    border: 2px solid ${props.theme.colors.accent.primary};
    
    &:hover {
      background: ${props.theme.colors.accent.primary};
      color: #000;
    }
  `}
`;

const FeatureGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 32px;
  margin-top: 80px;
  max-width: 1200px;
  margin-left: auto;
  margin-right: auto;
`;

const FeatureCard = styled.div`
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 32px;
  text-align: center;
`;

const FeatureIcon = styled.div`
  font-size: 48px;
  margin-bottom: 16px;
`;

const FeatureTitle = styled.h3`
  font-size: 20px;
  font-weight: 700;
  margin-bottom: 12px;
  color: ${props => props.theme.colors.text.primary};
`;

const FeatureDescription = styled.p`
  font-size: 16px;
  color: ${props => props.theme.colors.text.secondary};
  line-height: 1.6;
`;

export const Hero: React.FC = () => {
  const features = [
    {
      icon: '⚡',
      title: 'High Leverage Trading',
      description: 'Trade with up to 500x leverage on prediction markets'
    },
    {
      icon: '🔗',
      title: 'Chain Positions',
      description: 'Build complex strategies across multiple markets'
    },
    {
      icon: '🌊',
      title: 'Deep Liquidity',
      description: 'Access aggregated liquidity from multiple sources'
    }
  ];
  
  return (
    <HeroSection data-testid="hero-section">
      <HeroContent>
        <HeroTitle>
          Trade Prediction Markets<br />
          With Extreme Leverage
        </HeroTitle>
        
        <HeroSubtitle>
          The most powerful prediction market platform on Solana.
          Trade with confidence using advanced order types and up to 500x leverage.
        </HeroSubtitle>
        
        <CTAContainer>
          <Link href="/markets" passHref legacyBehavior>
            <a style={{ textDecoration: 'none' }}>
              <CTAButton primary>
                Start Trading
              </CTAButton>
            </a>
          </Link>
          <Link href="/demo" passHref legacyBehavior>
            <a style={{ textDecoration: 'none' }}>
              <CTAButton>
                Try Demo Mode
              </CTAButton>
            </a>
          </Link>
        </CTAContainer>
        
        <FeatureGrid>
          {features.map((feature, index) => (
            <FeatureCard key={index}>
              <FeatureIcon>{feature.icon}</FeatureIcon>
              <FeatureTitle>{feature.title}</FeatureTitle>
              <FeatureDescription>{feature.description}</FeatureDescription>
            </FeatureCard>
          ))}
        </FeatureGrid>
      </HeroContent>
    </HeroSection>
  );
};