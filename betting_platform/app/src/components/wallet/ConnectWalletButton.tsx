import React, { useState, useCallback, useEffect } from 'react';
import styled from '@emotion/styled';
import { useWallet } from '@solana/wallet-adapter-react';
import { useWalletModal } from '@solana/wallet-adapter-react-ui';

const Button = styled.button<{ connected?: boolean }>`
  background: ${props => props.connected ? 
    props.theme.colors.background.tertiary : 
    props.theme.colors.accent.primary};
  color: ${props => props.connected ? 
    props.theme.colors.text.primary : 
    '#000'};
  border: none;
  border-radius: 8px;
  padding: 12px 24px;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }
  
  &:active {
    transform: translateY(0);
  }
`;

const WalletAddress = styled.span`
  font-family: ${props => props.theme.typography.fonts.mono};
  font-size: 14px;
`;

const WalletMenu = styled.div`
  position: relative;
  display: inline-block;
`;

const Dropdown = styled.div<{ open: boolean }>`
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 8px;
  background: ${props => props.theme.colors.background.secondary};
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 8px;
  min-width: 200px;
  display: ${props => props.open ? 'block' : 'none'};
  z-index: 1000;
`;

const MenuItem = styled.button`
  display: block;
  width: 100%;
  padding: 12px 16px;
  background: none;
  border: none;
  color: ${props => props.theme.colors.text.primary};
  font-size: 14px;
  text-align: left;
  cursor: pointer;
  border-radius: 4px;
  
  &:hover {
    background: rgba(255, 255, 255, 0.05);
  }
`;

const DemoButton = styled(Button)`
  background: ${props => props.theme.colors.background.tertiary};
  color: ${props => props.theme.colors.text.primary};
  border: 1px solid ${props => props.theme.colors.accent.primary};
`;

const DemoBadge = styled.span`
  background: ${props => props.theme.colors.accent.secondary};
  color: #000;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 700;
  margin-left: 8px;
`;

export const ConnectWalletButton: React.FC = () => {
  const { publicKey, disconnect, wallet } = useWallet();
  const { setVisible } = useWalletModal();
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [isDemoMode, setIsDemoMode] = useState(false);
  
  const formatAddress = (address: string) => {
    return `${address.slice(0, 4)}...${address.slice(-4)}`;
  };
  
  const handleConnect = useCallback(() => {
    setVisible(true);
  }, [setVisible]);
  
  const handleDisconnect = useCallback(() => {
    disconnect();
    setDropdownOpen(false);
    setIsDemoMode(false);
  }, [disconnect]);
  
  const handleDemoMode = useCallback(async () => {
    // Create demo account
    try {
      const response = await fetch('/api/wallet/demo/create', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initial_balance: 10000 })
      });
      
      if (response.ok) {
        const data = await response.json();
        setIsDemoMode(true);
        // Store demo wallet in session
        sessionStorage.setItem('demoWallet', JSON.stringify(data));
      }
    } catch (error) {
      console.error('Failed to create demo account:', error);
    }
  }, []);
  
  // Check for demo mode on mount
  useEffect(() => {
    const demoWallet = sessionStorage.getItem('demoWallet');
    if (demoWallet) {
      setIsDemoMode(true);
    }
  }, []);
  
  if (publicKey || isDemoMode) {
    const address = publicKey?.toBase58() || JSON.parse(sessionStorage.getItem('demoWallet') || '{}').wallet_address;
    
    return (
      <WalletMenu data-testid="wallet-menu">
        <Button 
          connected 
          onClick={() => setDropdownOpen(!dropdownOpen)}
          data-testid="wallet-address"
        >
          <WalletAddress>
            {formatAddress(address || '')}
            {isDemoMode && <DemoBadge data-testid="demo-badge">DEMO</DemoBadge>}
          </WalletAddress>
        </Button>
        
        <Dropdown open={dropdownOpen}>
          <MenuItem onClick={() => setDropdownOpen(false)}>
            View Portfolio
          </MenuItem>
          <MenuItem onClick={() => setDropdownOpen(false)}>
            Transaction History
          </MenuItem>
          <MenuItem onClick={handleDisconnect}>
            Disconnect
          </MenuItem>
        </Dropdown>
      </WalletMenu>
    );
  }
  
  return (
    <>
      <Button 
        onClick={handleConnect}
        data-testid="connect-wallet"
      >
        Connect Wallet
      </Button>
      <DemoButton 
        onClick={handleDemoMode}
        data-testid="demo-mode"
      >
        Try Demo
      </DemoButton>
    </>
  );
};