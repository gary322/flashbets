import React, { createContext, useContext, useMemo, ReactNode } from 'react';
import { Connection, clusterApiUrl, Commitment } from '@solana/web3.js';
import AsyncStorage from '@react-native-async-storage/async-storage';

interface SolanaContextType {
  connection: Connection;
  network: 'mainnet-beta' | 'testnet' | 'devnet';
  changeNetwork: (network: 'mainnet-beta' | 'testnet' | 'devnet') => void;
}

const SolanaContext = createContext<SolanaContextType | undefined>(undefined);

const RPC_ENDPOINTS = {
  'mainnet-beta': 'https://api.mainnet-beta.solana.com',
  'testnet': 'https://api.testnet.solana.com',
  'devnet': 'https://api.devnet.solana.com',
};

export const SolanaProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [network, setNetwork] = React.useState<'mainnet-beta' | 'testnet' | 'devnet'>('mainnet-beta');

  React.useEffect(() => {
    // Load saved network preference
    AsyncStorage.getItem('solana-network').then(saved => {
      if (saved && ['mainnet-beta', 'testnet', 'devnet'].includes(saved)) {
        setNetwork(saved as any);
      }
    });
  }, []);

  const connection = useMemo(() => {
    const commitment: Commitment = 'confirmed';
    return new Connection(RPC_ENDPOINTS[network], {
      commitment,
      wsEndpoint: RPC_ENDPOINTS[network].replace('https', 'wss'),
    });
  }, [network]);

  const changeNetwork = async (newNetwork: 'mainnet-beta' | 'testnet' | 'devnet') => {
    setNetwork(newNetwork);
    await AsyncStorage.setItem('solana-network', newNetwork);
  };

  const value = {
    connection,
    network,
    changeNetwork,
  };

  return (
    <SolanaContext.Provider value={value}>
      {children}
    </SolanaContext.Provider>
  );
};

export const useSolana = (): SolanaContextType => {
  const context = useContext(SolanaContext);
  if (!context) {
    throw new Error('useSolana must be used within SolanaProvider');
  }
  return context;
};