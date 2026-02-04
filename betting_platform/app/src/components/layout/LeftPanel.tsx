import React from 'react';
import styled from '@emotion/styled';
import { useWallet } from '@solana/wallet-adapter-react';
import { useWalletModal } from '@solana/wallet-adapter-react-ui';
import { Connection, LAMPORTS_PER_SOL } from '@solana/web3.js';
import { formatAddress } from '../../utils/format';

interface LeftPanelProps {
  verseTree?: React.ReactNode;
  positions?: React.ReactNode;
}

const PanelContainer = styled.div`
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
`;

const PanelHeader = styled.div`
  padding: 24px 20px;
  border-bottom: 1px solid ${props => props.theme.colors.text.tertiary};
`;

const Logo = styled.div`
  font-size: 20px;
  font-weight: 600;
  letter-spacing: -0.5px;
  background: linear-gradient(135deg, ${props => props.theme.colors.accent.primary} 0%, ${props => props.theme.colors.accent.secondary} 100%);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
`;

const WalletSection = styled.div`
  padding: 20px;
  border-bottom: 1px solid ${props => props.theme.colors.text.tertiary};
`;

const WalletCard = styled.div`
  background: rgba(255, 255, 255, 0.03);
  border-radius: 12px;
  padding: 16px;
`;

const WalletStatus = styled.div`
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
`;

const WalletIndicator = styled.div<{ connected: boolean }>`
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: ${props => props.connected ? props.theme.colors.status.success : props.theme.colors.status.error};
`;

const WalletStatusText = styled.span`
  font-size: 14px;
  color: ${props => props.theme.colors.text.secondary};
`;

const ConnectWalletBtn = styled.button`
  width: 100%;
  padding: 12px;
  background: linear-gradient(135deg, ${props => props.theme.colors.accent.primary} 0%, ${props => props.theme.colors.accent.secondary} 100%);
  border: none;
  border-radius: 8px;
  color: ${props => props.theme.colors.text.inverse};
  font-weight: 600;
  cursor: pointer;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  
  &:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(255, 214, 10, 0.3);
  }
  
  &:active {
    transform: translateY(0);
  }
`;

const WalletAddress = styled.div`
  font-size: 13px;
  color: ${props => props.theme.colors.text.secondary};
  margin-top: 8px;
  font-family: ${props => props.theme.typography.fonts.mono};
`;

const BalanceDisplay = styled.div`
  margin-top: 12px;
`;

const BalanceLabel = styled.div`
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${props => props.theme.colors.text.tertiary};
  margin-bottom: 4px;
`;

const BalanceAmount = styled.div`
  font-size: 24px;
  font-weight: 300;
  color: ${props => props.theme.colors.text.primary};
`;

const SearchSection = styled.div`
  padding: 20px;
  border-bottom: 1px solid ${props => props.theme.colors.text.tertiary};
`;

const SearchLabel = styled.div`
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: ${props => props.theme.colors.text.tertiary};
  margin-bottom: 8px;
`;

const SearchInputContainer = styled.div`
  position: relative;
`;

const SearchInput = styled.input`
  width: 100%;
  padding: 12px 40px 12px 16px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: ${props => props.theme.colors.text.primary};
  font-size: 14px;
  transition: all ${props => props.theme.animation.durations.fast} ${props => props.theme.animation.easings.default};
  
  &:focus {
    outline: none;
    border-color: ${props => props.theme.colors.accent.primary};
    background: rgba(255, 255, 255, 0.08);
  }
  
  &::placeholder {
    color: ${props => props.theme.colors.text.tertiary};
  }
`;

const SearchIcon = styled.div`
  position: absolute;
  right: 16px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 16px;
  opacity: 0.5;
`;

const PositionsSection = styled.div`
  padding: 20px;
  border-bottom: 1px solid ${props => props.theme.colors.text.tertiary};
`;

const SectionHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
`;

const SectionTitle = styled.h3`
  font-size: 16px;
  font-weight: 600;
  margin: 0;
`;

const PositionsSummary = styled.div`
  display: flex;
  gap: 16px;
  font-size: 12px;
`;

const SummaryItem = styled.span`
  color: ${props => props.theme.colors.text.secondary};
  
  .value {
    color: ${props => props.theme.colors.text.primary};
    font-weight: 500;
  }
  
  .positive {
    color: ${props => props.theme.colors.status.success};
  }
  
  .negative {
    color: ${props => props.theme.colors.status.error};
  }
`;

const VerseNavigation = styled.div`
  flex: 1;
  overflow-y: auto;
  padding: 20px;
`;

const NoPositions = styled.div`
  text-align: center;
  padding: 40px 20px;
  color: ${props => props.theme.colors.text.tertiary};
  font-size: 14px;
`;

export default function LeftPanel({ verseTree, positions }: LeftPanelProps) {
  const { publicKey, connected, disconnect } = useWallet();
  const { setVisible } = useWalletModal();
  const [balance, setBalance] = React.useState(0);
  const [searchQuery, setSearchQuery] = React.useState('');

  React.useEffect(() => {
    if (publicKey) {
      const connection = new Connection(process.env.NEXT_PUBLIC_RPC_URL || 'https://api.mainnet-beta.solana.com');
      connection.getBalance(publicKey).then(lamports => {
        setBalance(lamports / LAMPORTS_PER_SOL);
      });
    }
  }, [publicKey]);

  const handleConnect = () => {
    setVisible(true);
  };

  const handleDisconnect = () => {
    disconnect();
  };

  return (
    <PanelContainer>
      <PanelHeader>
        <Logo>Quantum Platform</Logo>
      </PanelHeader>

      <WalletSection>
        <WalletCard>
          <WalletStatus>
            <WalletIndicator connected={connected} />
            <WalletStatusText>
              {connected ? 'Connected' : 'Not Connected'}
            </WalletStatusText>
          </WalletStatus>
          
          {!connected ? (
            <ConnectWalletBtn onClick={handleConnect}>
              Connect Wallet
            </ConnectWalletBtn>
          ) : (
            <>
              <WalletAddress>
                {formatAddress(publicKey?.toBase58() || '')}
              </WalletAddress>
              <BalanceDisplay>
                <BalanceLabel>Balance</BalanceLabel>
                <BalanceAmount>{balance.toFixed(2)} SOL</BalanceAmount>
              </BalanceDisplay>
              <ConnectWalletBtn 
                onClick={handleDisconnect}
                style={{ 
                  marginTop: '12px', 
                  background: 'rgba(255, 255, 255, 0.1)' 
                }}
              >
                Disconnect
              </ConnectWalletBtn>
            </>
          )}
        </WalletCard>
      </WalletSection>

      <SearchSection>
        <SearchLabel>Search Markets</SearchLabel>
        <SearchInputContainer>
          <SearchInput
            type="text"
            placeholder="Search for markets..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          <SearchIcon>🔍</SearchIcon>
        </SearchInputContainer>
      </SearchSection>

      {connected && positions && (
        <PositionsSection>
          <SectionHeader>
            <SectionTitle>My Positions</SectionTitle>
            <PositionsSummary>
              <SummaryItem>
                Open: <span className="value">0</span>
              </SummaryItem>
              <SummaryItem>
                P&L: <span className="positive">+0.00 SOL</span>
              </SummaryItem>
            </PositionsSummary>
          </SectionHeader>
          {positions}
        </PositionsSection>
      )}

      <VerseNavigation>
        {verseTree || (
          <NoPositions>
            Connect wallet to see verse navigation
          </NoPositions>
        )}
      </VerseNavigation>
    </PanelContainer>
  );
}