import React, { createContext, useContext, useEffect, useState } from 'react';
import { Alert } from 'react-native';
import WalletConnect from '@walletconnect/react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { PublicKey, Transaction } from '@solana/web3.js';
import * as Keychain from 'react-native-keychain';

interface WalletContextType {
  connected: boolean;
  publicKey: PublicKey | null;
  connecting: boolean;
  disconnect: () => Promise<void>;
  connect: () => Promise<void>;
  signTransaction: (transaction: Transaction) => Promise<Transaction>;
  signAllTransactions: (transactions: Transaction[]) => Promise<Transaction[]>;
  signMessage: (message: Uint8Array) => Promise<Uint8Array>;
}

const WalletContext = createContext<WalletContextType | undefined>(undefined);

// WalletConnect v2 configuration
const WALLET_CONNECT_PROJECT_ID = 'YOUR_PROJECT_ID'; // Replace with actual project ID
const WALLET_CONNECT_METADATA = {
  name: 'Betting Platform',
  description: 'Native Solana betting platform with advanced features',
  url: 'https://betting-platform.io',
  icons: ['https://betting-platform.io/icon.png'],
};

export const WalletProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [connected, setConnected] = useState(false);
  const [publicKey, setPublicKey] = useState<PublicKey | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [walletConnect, setWalletConnect] = useState<WalletConnect | null>(null);

  useEffect(() => {
    initializeWalletConnect();
  }, []);

  const initializeWalletConnect = async () => {
    try {
      const wc = new WalletConnect({
        bridge: 'https://bridge.walletconnect.org',
        clientMeta: WALLET_CONNECT_METADATA,
        redirectUrl: 'bettingplatform://',
        storageOptions: {
          asyncStorage: AsyncStorage,
        },
      });

      // Check if there's an existing session
      if (wc.connected) {
        const accounts = wc.accounts;
        if (accounts.length > 0) {
          const pubKey = new PublicKey(accounts[0]);
          setPublicKey(pubKey);
          setConnected(true);
          
          // Store in secure keychain
          await Keychain.setInternetCredentials(
            'betting-platform-wallet',
            accounts[0],
            'wallet-session'
          );
        }
      }

      // Set up event listeners
      wc.on('connect', async (error, payload) => {
        if (error) {
          Alert.alert('Connection Error', error.message);
          return;
        }

        const accounts = payload.params[0].accounts;
        if (accounts.length > 0) {
          const pubKey = new PublicKey(accounts[0]);
          setPublicKey(pubKey);
          setConnected(true);
          
          await Keychain.setInternetCredentials(
            'betting-platform-wallet',
            accounts[0],
            'wallet-session'
          );
        }
      });

      wc.on('disconnect', () => {
        setConnected(false);
        setPublicKey(null);
        Keychain.resetInternetCredentials('betting-platform-wallet');
      });

      setWalletConnect(wc);
    } catch (error) {
      console.error('Failed to initialize WalletConnect:', error);
    }
  };

  const connect = async () => {
    if (!walletConnect || connecting) return;

    try {
      setConnecting(true);

      if (!walletConnect.connected) {
        // Create new session
        await walletConnect.createSession({
          chainId: 1, // Solana chain ID
        });
      }
    } catch (error) {
      Alert.alert('Connection Failed', 'Unable to connect to wallet');
      console.error('Connection error:', error);
    } finally {
      setConnecting(false);
    }
  };

  const disconnect = async () => {
    if (!walletConnect) return;

    try {
      await walletConnect.killSession();
      setConnected(false);
      setPublicKey(null);
      await Keychain.resetInternetCredentials('betting-platform-wallet');
    } catch (error) {
      console.error('Disconnect error:', error);
    }
  };

  const signTransaction = async (transaction: Transaction): Promise<Transaction> => {
    if (!walletConnect || !connected) {
      throw new Error('Wallet not connected');
    }

    try {
      const message = transaction.serializeMessage();
      const signature = await walletConnect.signTransaction({
        transaction: message.toString('base64'),
      });

      transaction.addSignature(publicKey!, Buffer.from(signature, 'base64'));
      return transaction;
    } catch (error) {
      console.error('Sign transaction error:', error);
      throw error;
    }
  };

  const signAllTransactions = async (
    transactions: Transaction[]
  ): Promise<Transaction[]> => {
    return Promise.all(transactions.map(tx => signTransaction(tx)));
  };

  const signMessage = async (message: Uint8Array): Promise<Uint8Array> => {
    if (!walletConnect || !connected) {
      throw new Error('Wallet not connected');
    }

    try {
      const signature = await walletConnect.signMessage({
        message: Buffer.from(message).toString('base64'),
      });
      return Buffer.from(signature, 'base64');
    } catch (error) {
      console.error('Sign message error:', error);
      throw error;
    }
  };

  const value: WalletContextType = {
    connected,
    publicKey,
    connecting,
    connect,
    disconnect,
    signTransaction,
    signAllTransactions,
    signMessage,
  };

  return (
    <WalletContext.Provider value={value}>{children}</WalletContext.Provider>
  );
};

export const useWallet = (): WalletContextType => {
  const context = useContext(WalletContext);
  if (!context) {
    throw new Error('useWallet must be used within WalletProvider');
  }
  return context;
};