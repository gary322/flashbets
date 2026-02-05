import { createContext, useContext, useState, useEffect, useCallback, ReactNode, useRef } from 'react';
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
  const demoWalletEnabled = process.env.NEXT_PUBLIC_DEMO_WALLET_ENABLED === 'true';
  const demoWalletRef = useRef<ethers.Wallet | null>(null);
  const [wallet, setWallet] = useState<WalletState>({
    isConnected: false,
    address: null,
    chainId: null,
    balance: null
  });
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const getOrCreateDemoWallet = useCallback((): ethers.Wallet => {
    if (demoWalletRef.current) return demoWalletRef.current;

    // Persist across reloads for a smoother demo experience.
    if (typeof window !== 'undefined') {
      const storageKey = 'flashbets_demo_wallet_private_key_v1';
      let privateKey = window.localStorage.getItem(storageKey);
      if (!privateKey) {
        privateKey = ethers.Wallet.createRandom().privateKey;
        window.localStorage.setItem(storageKey, privateKey);
      }

      demoWalletRef.current = new ethers.Wallet(privateKey);
      return demoWalletRef.current;
    }

    // SSR fallback (shouldn't generally be used for signing).
    demoWalletRef.current = ethers.Wallet.createRandom();
    return demoWalletRef.current;
  }, []);

  // Check if already connected on mount
  useEffect(() => {
    if (demoWalletEnabled) return;

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
  }, [demoWalletEnabled]);

  // Listen to account and chain changes
  useEffect(() => {
    if (demoWalletEnabled) return;
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
  }, [demoWalletEnabled]);

  const connect = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      if (demoWalletEnabled) {
        const demoWallet = getOrCreateDemoWallet();
        setWallet({
          isConnected: true,
          address: demoWallet.address,
          chainId: 137,
          balance: '0',
        });
        return;
      }

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
  }, [demoWalletEnabled, getOrCreateDemoWallet]);

  const disconnect = useCallback(() => {
    if (!demoWalletEnabled) {
      metamask.disconnect();
    }
    setWallet({
      isConnected: false,
      address: null,
      chainId: null,
      balance: null
    });
  }, [demoWalletEnabled]);

  const switchToPolygon = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (demoWalletEnabled) return;
      await metamask.switchToPolygon();
    } catch (err: any) {
      setError(err.message || 'Failed to switch network');
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, [demoWalletEnabled]);

  const signOrder = useCallback(async (orderData: any) => {
    setError(null);
    
    try {
      if (!wallet.isConnected) {
        throw new Error('Wallet not connected');
      }

      if (demoWalletEnabled) {
        const demoWallet = getOrCreateDemoWallet();
        const domain = orderData?.domain;
        const types = orderData?.types || {};
        const message = orderData?.message;

        // ethers expects the EIP712Domain type to be omitted.
        const { EIP712Domain: _ignored, ...signTypes } = types;
        return await demoWallet._signTypedData(domain, signTypes, message);
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
  }, [wallet, demoWalletEnabled, getOrCreateDemoWallet, switchToPolygon]);

  const getUSDCBalance = useCallback(async () => {
    try {
      if (!wallet.isConnected) {
        throw new Error('Wallet not connected');
      }

      if (demoWalletEnabled) {
        return '100000.00';
      }

      const balance = await metamask.getUSDCBalance();
      return balance;
    } catch (err: any) {
      setError(err.message || 'Failed to get USDC balance');
      throw err;
    }
  }, [wallet, demoWalletEnabled]);

  const approveUSDC = useCallback(async (spender: string, amount: string) => {
    setError(null);
    
    try {
      if (!wallet.isConnected) {
        throw new Error('Wallet not connected');
      }

      if (demoWalletEnabled) {
        return `demo_approve_${spender}_${amount}_${Date.now()}`;
      }

      const txHash = await metamask.approveUSDC(spender, amount);
      return txHash;
    } catch (err: any) {
      setError(err.message || 'Failed to approve USDC');
      throw err;
    }
  }, [wallet, demoWalletEnabled]);

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
