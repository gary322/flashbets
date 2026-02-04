import React from 'react';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { createStackNavigator } from '@react-navigation/stack';
import Icon from 'react-native-vector-icons/MaterialIcons';

// Screens
import MarketsScreen from '@screens/MarketsScreen';
import PositionsScreen from '@screens/PositionsScreen';
import ChainBuilderScreen from '@screens/ChainBuilderScreen';
import L2DistributionScreen from '@screens/L2DistributionScreen';
import ProfileScreen from '@screens/ProfileScreen';

// Trading Stack
import TradingDetailScreen from '@screens/trading/TradingDetailScreen';
import OrderBookScreen from '@screens/trading/OrderBookScreen';
import ChartScreen from '@screens/trading/ChartScreen';

// Auth Stack
import OnboardingScreen from '@screens/auth/OnboardingScreen';
import ConnectWalletScreen from '@screens/auth/ConnectWalletScreen';

// Hooks
import { useWallet } from '@hooks/useWallet';

const Tab = createBottomTabNavigator();
const Stack = createStackNavigator();

// Trading Stack Navigator
const TradingStack = () => (
  <Stack.Navigator
    screenOptions={{
      headerShown: false,
    }}>
    <Stack.Screen name="Markets" component={MarketsScreen} />
    <Stack.Screen name="TradingDetail" component={TradingDetailScreen} />
    <Stack.Screen name="OrderBook" component={OrderBookScreen} />
    <Stack.Screen name="Chart" component={ChartScreen} />
  </Stack.Navigator>
);

// Main Tab Navigator
const MainTabs = () => (
  <Tab.Navigator
    screenOptions={{
      tabBarActiveTintColor: '#00D4FF',
      tabBarInactiveTintColor: '#666',
      tabBarStyle: {
        backgroundColor: '#0F0F0F',
        borderTopColor: '#1a1a1a',
        paddingBottom: 5,
        height: 60,
      },
      headerShown: false,
    }}>
    <Tab.Screen
      name="Trading"
      component={TradingStack}
      options={{
        tabBarLabel: 'Markets',
        tabBarIcon: ({ color, size }) => (
          <Icon name="trending-up" size={size} color={color} />
        ),
      }}
    />
    <Tab.Screen
      name="Positions"
      component={PositionsScreen}
      options={{
        tabBarLabel: 'Positions',
        tabBarIcon: ({ color, size }) => (
          <Icon name="account-balance-wallet" size={size} color={color} />
        ),
      }}
    />
    <Tab.Screen
      name="ChainBuilder"
      component={ChainBuilderScreen}
      options={{
        tabBarLabel: 'Chains',
        tabBarIcon: ({ color, size }) => (
          <Icon name="link" size={size} color={color} />
        ),
      }}
    />
    <Tab.Screen
      name="L2Distribution"
      component={L2DistributionScreen}
      options={{
        tabBarLabel: 'L2 AMM',
        tabBarIcon: ({ color, size }) => (
          <Icon name="analytics" size={size} color={color} />
        ),
      }}
    />
    <Tab.Screen
      name="Profile"
      component={ProfileScreen}
      options={{
        tabBarLabel: 'Profile',
        tabBarIcon: ({ color, size }) => (
          <Icon name="person" size={size} color={color} />
        ),
      }}
    />
  </Tab.Navigator>
);

// Root Navigator
const RootNavigator: React.FC = () => {
  const { connected } = useWallet();

  return (
    <Stack.Navigator
      screenOptions={{
        headerShown: false,
      }}>
      {!connected ? (
        <>
          <Stack.Screen name="Onboarding" component={OnboardingScreen} />
          <Stack.Screen name="ConnectWallet" component={ConnectWalletScreen} />
        </>
      ) : (
        <Stack.Screen name="Main" component={MainTabs} />
      )}
    </Stack.Navigator>
  );
};

export default RootNavigator;