import React from 'react';
import Head from 'next/head';
import styled from '@emotion/styled';
import { useRouter } from 'next/router';

const DemoContainer = styled.div`
  max-width: 800px;
  margin: 0 auto;
  padding: 80px 24px;
  text-align: center;
`;

const Title = styled.h1`
  font-size: 48px;
  font-weight: 900;
  margin-bottom: 24px;
  color: ${props => props.theme.colors.text.primary};
`;

const Subtitle = styled.p`
  font-size: 20px;
  color: ${props => props.theme.colors.text.secondary};
  margin-bottom: 48px;
  line-height: 1.6;
`;

const DemoCard = styled.div`
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 16px;
  padding: 48px;
  margin-bottom: 32px;
`;

const DemoBalance = styled.div`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 16px;
`;

const BalanceAmount = styled.div`
  font-size: 48px;
  font-weight: 800;
  color: ${props => props.theme.colors.accent.primary};
  margin-bottom: 32px;
`;

const CreateButton = styled.button`
  padding: 16px 48px;
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
  
  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
`;

const Features = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 24px;
  margin-top: 48px;
`;

const Feature = styled.div`
  background: rgba(0, 255, 136, 0.05);
  border: 1px solid rgba(0, 255, 136, 0.2);
  border-radius: 12px;
  padding: 24px;
`;

const FeatureIcon = styled.div`
  font-size: 32px;
  margin-bottom: 12px;
`;

const FeatureTitle = styled.h3`
  font-size: 18px;
  font-weight: 600;
  margin-bottom: 8px;
  color: ${props => props.theme.colors.text.primary};
`;

const FeatureText = styled.p`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
  line-height: 1.5;
`;

const InfoBox = styled.div`
  background: rgba(255, 193, 7, 0.1);
  border: 1px solid rgba(255, 193, 7, 0.3);
  border-radius: 8px;
  padding: 16px;
  margin-top: 32px;
  text-align: left;
  font-size: 14px;
  color: #FFC107;
`;

const LoadingState = styled.div`
  padding: 24px;
  color: ${props => props.theme.colors.text.secondary};
`;

const ErrorState = styled.div`
  background: rgba(255, 0, 0, 0.1);
  border: 1px solid rgba(255, 0, 0, 0.3);
  border-radius: 8px;
  padding: 24px;
  color: #ff6b6b;
  margin-bottom: 24px;
`;

const SuccessState = styled.div`
  background: rgba(0, 255, 136, 0.1);
  border: 1px solid rgba(0, 255, 136, 0.3);
  border-radius: 8px;
  padding: 24px;
  color: #00FF88;
  margin-bottom: 24px;
`;

const WalletInfo = styled.div`
  background: ${props => props.theme.colors.background.primary};
  border-radius: 8px;
  padding: 16px;
  margin-top: 24px;
  font-family: monospace;
  font-size: 14px;
  word-break: break-all;
  
  div {
    margin-bottom: 8px;
    
    &:last-child {
      margin-bottom: 0;
    }
  }
  
  strong {
    color: ${props => props.theme.colors.text.secondary};
  }
`;

export default function Demo() {
  const router = useRouter();
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [demoAccount, setDemoAccount] = React.useState<any>(null);

  const createDemoAccount = async () => {
    try {
      setLoading(true);
      setError(null);
      
      const response = await fetch('/api/wallet/demo/create', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
      });
      
      if (!response.ok) {
        throw new Error('Failed to create demo account');
      }
      
      const data = await response.json();
      setDemoAccount(data);
      
      // Store in localStorage
      localStorage.setItem('demoWallet', JSON.stringify(data));
      
      // Redirect to markets after 3 seconds
      setTimeout(() => {
        router.push('/markets');
      }, 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Something went wrong');
    } finally {
      setLoading(false);
    }
  };

  const features = [
    {
      icon: '💰',
      title: '10,000 SOL Demo Balance',
      text: 'Start with plenty of funds to explore all trading features'
    },
    {
      icon: '🚀',
      title: 'Full Platform Access',
      text: 'Experience all features including high leverage trading'
    },
    {
      icon: '📊',
      title: 'Real Market Data',
      text: 'Trade on live markets with real-time price updates'
    }
  ];

  return (
    <>
      <Head>
        <title>Demo Mode - Betting Platform</title>
        <meta name="description" content="Try the platform with demo funds" />
      </Head>

      <DemoContainer>
        <Title>Demo Trading Mode</Title>
        <Subtitle>
          Experience the full power of our platform with zero risk. 
          Start trading with 10,000 SOL in demo funds instantly.
        </Subtitle>

        {error && (
          <ErrorState>{error}</ErrorState>
        )}

        {demoAccount ? (
          <>
            <SuccessState>
              ✅ Demo account created successfully! Redirecting to markets...
            </SuccessState>
            <DemoCard>
              <DemoBalance>Your Demo Wallet</DemoBalance>
              <WalletInfo>
                <div>
                  <strong>Address:</strong> {demoAccount.wallet_address || demoAccount.wallet}
                </div>
                <div>
                  <strong>Private Key:</strong> {demoAccount.private_key || demoAccount.privateKey}
                </div>
              </WalletInfo>
              <InfoBox>
                <strong>⚠️ Important:</strong> This is a demo wallet for testing only. 
                Never send real funds to this address.
              </InfoBox>
            </DemoCard>
          </>
        ) : (
          <DemoCard>
            <DemoBalance>Demo Balance</DemoBalance>
            <BalanceAmount>10,000 SOL</BalanceAmount>
            <CreateButton onClick={createDemoAccount} disabled={loading}>
              {loading ? (
                <LoadingState>Creating Demo Account...</LoadingState>
              ) : (
                'Create Demo Account'
              )}
            </CreateButton>
          </DemoCard>
        )}

        <Features>
          {features.map((feature, index) => (
            <Feature key={index}>
              <FeatureIcon>{feature.icon}</FeatureIcon>
              <FeatureTitle>{feature.title}</FeatureTitle>
              <FeatureText>{feature.text}</FeatureText>
            </Feature>
          ))}
        </Features>

        <InfoBox>
          <strong>💡 Demo Mode Features:</strong>
          <ul style={{ marginTop: '8px', marginLeft: '20px' }}>
            <li>All trades are simulated - no real money involved</li>
            <li>Perfect for learning the platform and testing strategies</li>
            <li>Full access to all markets and trading features</li>
            <li>Performance tracked separately from real accounts</li>
          </ul>
        </InfoBox>
      </DemoContainer>
    </>
  );
}
