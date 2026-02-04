import React, { useState } from 'react';
import {
  View,
  Text,
  StyleSheet,
  FlatList,
  TouchableOpacity,
  RefreshControl,
  Alert,
} from 'react-native';
import { useNavigation } from '@react-navigation/native';
import Icon from 'react-native-vector-icons/MaterialIcons';
import { SwipeListView } from 'react-native-swipe-list-view';

// Components  
import { PositionCard } from '@components/positions/PositionCard';
import { PositionStats } from '@components/positions/PositionStats';
import { QuickActions } from '@components/positions/QuickActions';

// Hooks
import { usePositions } from '@hooks/usePositions';
import { useTheme } from '@hooks/useTheme';
import { useWallet } from '@hooks/useWallet';

// Types
import { Position } from '@types';

const PositionsScreen: React.FC = () => {
  const navigation = useNavigation();
  const { theme } = useTheme();
  const { connected } = useWallet();
  const { 
    positions, 
    loading, 
    refresh,
    closePosition,
    addToPosition,
    partialClose 
  } = usePositions();

  const [selectedTab, setSelectedTab] = useState<'active' | 'closed'>('active');

  const activePositions = positions.filter(p => !p.isClosed);
  const closedPositions = positions.filter(p => p.isClosed);
  const displayPositions = selectedTab === 'active' ? activePositions : closedPositions;

  const totalPnL = activePositions.reduce((sum, p) => sum + p.unrealizedPnL, 0);
  const totalValue = activePositions.reduce((sum, p) => sum + p.currentValue, 0);

  const handleClosePosition = async (position: Position) => {
    Alert.alert(
      'Close Position',
      `Are you sure you want to close your ${position.marketName} position?`,
      [
        { text: 'Cancel', style: 'cancel' },
        {
          text: 'Close',
          style: 'destructive',
          onPress: async () => {
            try {
              await closePosition(position.id);
              Alert.alert('Success', 'Position closed successfully');
            } catch (error) {
              Alert.alert('Error', error.message);
            }
          },
        },
      ]
    );
  };

  const renderPosition = ({ item }: { item: Position }) => (
    <PositionCard
      position={item}
      onPress={() => navigation.navigate('PositionDetail', { position: item })}
    />
  );

  const renderHiddenItem = ({ item }: { item: Position }) => (
    <View style={styles.hiddenContainer}>
      <TouchableOpacity
        style={[styles.hiddenButton, { backgroundColor: theme.warning }]}
        onPress={() => partialClose(item.id, 0.5)}>
        <Icon name="pie-chart" size={20} color="white" />
        <Text style={styles.hiddenText}>50%</Text>
      </TouchableOpacity>
      <TouchableOpacity
        style={[styles.hiddenButton, { backgroundColor: theme.error }]}
        onPress={() => handleClosePosition(item)}>
        <Icon name="close" size={20} color="white" />
        <Text style={styles.hiddenText}>Close</Text>
      </TouchableOpacity>
    </View>
  );

  if (!connected) {
    return (
      <View style={[styles.container, styles.centered, { backgroundColor: theme.background }]}>
        <Icon name="account-balance-wallet" size={64} color={theme.textSecondary} />
        <Text style={[styles.emptyText, { color: theme.textSecondary }]}>
          Connect your wallet to view positions
        </Text>
      </View>
    );
  }

  return (
    <View style={[styles.container, { backgroundColor: theme.background }]}>
      <PositionStats
        totalValue={totalValue}
        totalPnL={totalPnL}
        positionCount={activePositions.length}
      />

      <View style={styles.tabs}>
        <TouchableOpacity
          style={[
            styles.tab,
            selectedTab === 'active' && { borderBottomColor: theme.primary },
          ]}
          onPress={() => setSelectedTab('active')}>
          <Text
            style={[
              styles.tabText,
              { color: selectedTab === 'active' ? theme.primary : theme.textSecondary },
            ]}>
            Active ({activePositions.length})
          </Text>
        </TouchableOpacity>
        <TouchableOpacity
          style={[
            styles.tab,
            selectedTab === 'closed' && { borderBottomColor: theme.primary },
          ]}
          onPress={() => setSelectedTab('closed')}>
          <Text
            style={[
              styles.tabText,
              { color: selectedTab === 'closed' ? theme.primary : theme.textSecondary },
            ]}>
            History ({closedPositions.length})
          </Text>
        </TouchableOpacity>
      </View>

      <SwipeListView
        data={displayPositions}
        renderItem={renderPosition}
        renderHiddenItem={renderHiddenItem}
        rightOpenValue={-160}
        disableRightSwipe
        keyExtractor={item => item.id}
        refreshControl={
          <RefreshControl
            refreshing={loading}
            onRefresh={refresh}
            tintColor={theme.primary}
          />
        }
        ListEmptyComponent={
          <View style={styles.empty}>
            <Icon 
              name={selectedTab === 'active' ? 'trending-flat' : 'history'} 
              size={48} 
              color={theme.textSecondary} 
            />
            <Text style={[styles.emptyText, { color: theme.textSecondary }]}>
              No {selectedTab} positions
            </Text>
          </View>
        }
        contentContainerStyle={styles.listContent}
      />

      {selectedTab === 'active' && activePositions.length > 0 && (
        <QuickActions />
      )}
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  centered: {
    justifyContent: 'center',
    alignItems: 'center',
  },
  tabs: {
    flexDirection: 'row',
    borderBottomWidth: 1,
    borderBottomColor: 'rgba(255,255,255,0.1)',
  },
  tab: {
    flex: 1,
    paddingVertical: 16,
    alignItems: 'center',
    borderBottomWidth: 2,
    borderBottomColor: 'transparent',
  },
  tabText: {
    fontSize: 16,
    fontWeight: '600',
  },
  listContent: {
    paddingBottom: 100,
  },
  empty: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    paddingVertical: 100,
  },
  emptyText: {
    marginTop: 16,
    fontSize: 16,
  },
  hiddenContainer: {
    flexDirection: 'row',
    justifyContent: 'flex-end',
    alignItems: 'center',
    flex: 1,
  },
  hiddenButton: {
    justifyContent: 'center',
    alignItems: 'center',
    width: 80,
    height: '100%',
  },
  hiddenText: {
    color: 'white',
    fontSize: 12,
    marginTop: 4,
  },
});

export default PositionsScreen;