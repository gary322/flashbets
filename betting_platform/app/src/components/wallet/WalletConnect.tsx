import React from 'react';
import styled from '@emotion/styled';
import { useMetaMask } from '../../hooks/useMetaMask';

const WalletButton = styled.button<{ connected?: boolean }>`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-radius: 8px;
  border: none;
  background: ${props => props.connected ? 'rgba(0, 255, 136, 0.1)' : props.theme.colors.accent.primary};
  color: ${props => props.connected ? props.theme.colors.accent.primary : '#000'};
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }
  
  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
`;

const WalletInfo = styled.div`
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
`;

const Address = styled.div`
  font-size: 14px;
  font-family: monospace;
`;

const Balance = styled.div`
  font-size: 12px;
  opacity: 0.7;
`;

const ErrorMessage = styled.div`
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 8px;
  padding: 8px 12px;
  background: rgba(255, 59, 48, 0.1);
  border: 1px solid rgba(255, 59, 48, 0.3);
  border-radius: 6px;
  color: #FF3B30;
  font-size: 14px;
  white-space: nowrap;
`;

const NetworkBadge = styled.div<{ isCorrect: boolean }>`
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 12px;
  background: ${props => props.isCorrect ? 'rgba(0, 255, 136, 0.1)' : 'rgba(255, 178, 36, 0.1)'};
  color: ${props => props.isCorrect ? props.theme.colors.accent.primary : '#FFB224'};
`;

const WalletDropdown = styled.div`
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 8px;
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 8px;
  min-width: 200px;
  z-index: 1000;
`;

const DropdownItem = styled.button`
  display: block;
  width: 100%;
  padding: 8px 12px;
  text-align: left;
  background: none;
  border: none;
  color: ${props => props.theme.colors.text.primary};
  cursor: pointer;
  border-radius: 4px;
  transition: background 0.2s ease;
  
  &:hover {
    background: rgba(255, 255, 255, 0.05);
  }
`;

const Container = styled.div`
  position: relative;
`;

export function WalletConnect() {
  const { wallet, isLoading, error, connect, disconnect, switchToPolygon, getUSDCBalance } = useMetaMask();
  const [showDropdown, setShowDropdown] = React.useState(false);
  const [usdcBalance, setUsdcBalance] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (wallet.isConnected) {
      // Fetch USDC balance
      const fetchBalance = async () => {
        try {
          const balance = await getUSDCBalance();
          setUsdcBalance(balance);
        } catch (err) {
          console.error('Failed to fetch USDC balance:', err);
        }
      };
      fetchBalance();
    }
  }, [wallet.isConnected, wallet.address, getUSDCBalance]);

  const formatAddress = (address: string) => {
    return `${address.slice(0, 6)}...${address.slice(-4)}`;
  };

  const getNetworkName = (chainId: number | null) => {
    switch (chainId) {
      case 1: return 'Ethereum';
      case 137: return 'Polygon';
      case 80001: return 'Mumbai';
      default: return 'Unknown';
    }
  };

  if (!wallet.isConnected) {
    return (
      <Container>
        <WalletButton 
          onClick={connect}
          disabled={isLoading}
        >
          {isLoading ? 'Connecting...' : 'Connect Wallet'}
        </WalletButton>
        {error && <ErrorMessage>{error}</ErrorMessage>}
      </Container>
    );
  }

  return (
    <Container>
      <WalletButton 
        connected
        onClick={() => setShowDropdown(!showDropdown)}
      >
        <WalletInfo>
          <Address>{formatAddress(wallet.address!)}</Address>
          {usdcBalance && (
            <Balance>{parseFloat(usdcBalance).toFixed(2)} USDC</Balance>
          )}
        </WalletInfo>
        <NetworkBadge isCorrect={wallet.chainId === 137}>
          {getNetworkName(wallet.chainId)}
        </NetworkBadge>
      </WalletButton>

      {showDropdown && (
        <WalletDropdown>
          {wallet.chainId !== 137 && (
            <DropdownItem onClick={async () => {
              await switchToPolygon();
              setShowDropdown(false);
            }}>
              Switch to Polygon
            </DropdownItem>
          )}
          <DropdownItem onClick={() => {
            disconnect();
            setShowDropdown(false);
          }}>
            Disconnect
          </DropdownItem>
        </WalletDropdown>
      )}

      {error && <ErrorMessage>{error}</ErrorMessage>}
    </Container>
  );
}