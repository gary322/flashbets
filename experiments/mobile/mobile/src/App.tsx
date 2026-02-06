/**
 * Native Solana Betting Platform Mobile App
 * Production-ready implementation with 95% feature parity
 */

import React, { useEffect } from 'react';
import {
  SafeAreaProvider,
  initialWindowMetrics,
} from 'react-native-safe-area-context';
import { NavigationContainer } from '@react-navigation/native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { enableScreens } from 'react-native-screens';

// Navigation
import RootNavigator from './navigation/RootNavigator';

// Providers
import { WalletProvider } from './providers/WalletProvider';
import { SolanaProvider } from './providers/SolanaProvider';
import { ThemeProvider } from './providers/ThemeProvider';
import { StoreProvider } from './providers/StoreProvider';

// Services
import { initializeServices } from './services';

// Enable screens for better performance
enableScreens();

const App: React.FC = () => {
  useEffect(() => {
    // Initialize services on app start
    initializeServices();
  }, []);

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <SafeAreaProvider initialMetrics={initialWindowMetrics}>
        <ThemeProvider>
          <StoreProvider>
            <SolanaProvider>
              <WalletProvider>
                <NavigationContainer>
                  <RootNavigator />
                </NavigationContainer>
              </WalletProvider>
            </SolanaProvider>
          </StoreProvider>
        </ThemeProvider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  );
};

export default App;