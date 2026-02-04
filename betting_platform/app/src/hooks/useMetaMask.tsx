import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { metamask, WalletState } from '../lib/metamask';
import { ethers } from 'ethers';

interface MetaMaskContextType {
  wallet: WalletState;
  isLoading: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  switchToPolygon: () => Promise<void>;
  signOrder: (orderData: any) => Promise<string>;
  getUSDCBalance: () => Promise<string>;
  approveUSDC: (spender: string, amount: string) => Promise<string>;
}

const MetaMaskContext = createContext<MetaMaskContextType | undefined>(undefined);

export function MetaMaskProvider({ children }: { children: ReactNode }) {
  const [wallet, setWallet] = useState<WalletState>({
    isConnected: false,
    address: null,
    chainId: null,
    balance: null
  });
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Check if already connected on mount
  useEffect(() => {
    const checkConnection = async () => {
      if (metamask.isInstalled()) {
        const account = await metamask.getAccount();
        if (account) {
          try {
            const state = await metamask.connect();
            setWallet(state);
          } catch (err) {
            console.error('Failed to reconnect:', err);
          }
        }
      }
    };
    checkConnection();
  }, []);

  // Listen to account and chain changes
  useEffect(() => {
    if (!metamask.isInstalled()) return;

    const handleAccountsChanged = (accounts: string[]) => {
      if (accounts.length === 0) {
        setWallet({
          isConnected: false,
          address: null,
          chainId: null,
          balance: null
        });
      } else {
        setWallet(prev => ({
          ...prev,
          address: accounts[0]
        }));
      }
    };

    const handleChainChanged = (chainId: string) => {
      setWallet(prev => ({
        ...prev,
        chainId: parseInt(chainId, 16)
      }));
    };

    metamask.onAccountsChanged(handleAccountsChanged);
    metamask.onChainChanged(handleChainChanged);

    return () => {
      metamask.removeAllListeners();
    };
  }, []);

  const connect = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      if (!metamask.isInstalled()) {
        throw new Error('Please install MetaMask to continue');
      }

      const state = await metamask.connect();
      setWallet(state);

      // Switch to Polygon if not already on it
      if (state.chainId !== 137) {
        await metamask.switchToPolygon();
      }
    } catch (err: any) {
      setError(err.message || 'Failed to connect wallet');
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    metamask.disconnect();
    setWallet({
      isConnected: false,
      address: null,
      chainId: null,
      balance: null
    });
  }, []);

  const switchToPolygon = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      await metamask.switchToPolygon();
    } catch (err: any) {
      setError(err.message || 'Failed to switch network');
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const signOrder = useCallback(async (orderData: any) => {
    setError(null);
    
    try {
      if (!wallet.isConnected) {
        throw new Error('Wallet not connected');
      }

      // Ensure we're on Polygon
      if (wallet.chainId !== 137) {
        await switchToPolygon();
      }

      const signature = await metamask.signTypedData(orderData);
      return signature;
    } catch (err: any) {
      setError(err.message || 'Failed to sign order');
      throw err;
    }
  }, [wallet, switchToPolygon]);

  const getUSDCBalance = useCallback(async () => {
    try {
      if (!wallet.isConnected) {
        throw new Error('Wallet not connected');
      }

      const balance = await metamask.getUSDCBalance();
      return balance;
    } catch (err: any) {
      setError(err.message || 'Failed to get USDC balance');
      throw err;
    }
  }, [wallet]);

  const approveUSDC = useCallback(async (spender: string, amount: string) => {
    setError(null);
    
    try {
      if (!wallet.isConnected) {
        throw new Error('Wallet not connected');
      }

      const txHash = await metamask.approveUSDC(spender, amount);
      return txHash;
    } catch (err: any) {
      setError(err.message || 'Failed to approve USDC');
      throw err;
    }
  }, [wallet]);

  const value: MetaMaskContextType = {
    wallet,
    isLoading,
    error,
    connect,
    disconnect,
    switchToPolygon,
    signOrder,
    getUSDCBalance,
    approveUSDC
  };

  return (
    <MetaMaskContext.Provider value={value}>
      {children}
    </MetaMaskContext.Provider>
  );
}

export function useMetaMask() {
  const context = useContext(MetaMaskContext);
  if (!context) {
    throw new Error('useMetaMask must be used within MetaMaskProvider');
  }
  return context;
}