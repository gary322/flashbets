import React, { useState, useEffect } from 'react';
import {
  View,
  Text,
  StyleSheet,
  FlatList,
  TouchableOpacity,
  TextInput,
  RefreshControl,
  ActivityIndicator,
} from 'react-native';
import { useNavigation } from '@react-navigation/native';
import Icon from 'react-native-vector-icons/MaterialIcons';

// Components
import { MarketCard } from '@components/markets/MarketCard';
import { FilterModal } from '@components/markets/FilterModal';
import { CategoryTabs } from '@components/markets/CategoryTabs';

// Hooks
import { useMarkets } from '@hooks/useMarkets';
import { useTheme } from '@hooks/useTheme';
import { usePolymarketWebSocket } from '@hooks/usePolymarketWebSocket';

// Types
import { Market, MarketCategory } from '@types';

const MarketsScreen: React.FC = () => {
  const navigation = useNavigation();
  const { theme } = useTheme();
  const { markets, loading, refresh, searchMarkets } = useMarkets();
  const { subscribeToMarket } = usePolymarketWebSocket();

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<MarketCategory>('all');
  const [showFilters, setShowFilters] = useState(false);
  const [filteredMarkets, setFilteredMarkets] = useState(markets);

  useEffect(() => {
    // Filter markets based on search and category
    let filtered = markets;

    if (searchQuery) {
      filtered = searchMarkets(searchQuery);
    }

    if (selectedCategory !== 'all') {
      filtered = filtered.filter(m => m.category === selectedCategory);
    }

    setFilteredMarkets(filtered);
  }, [markets, searchQuery, selectedCategory]);

  const handleMarketPress = (market: Market) => {
    // Subscribe to real-time updates
    subscribeToMarket(market.id);
    
    navigation.navigate('TradingDetail', { market });
  };

  const renderMarket = ({ item }: { item: Market }) => (
    <MarketCard
      market={item}
      onPress={() => handleMarketPress(item)}
      style={styles.marketCard}
    />
  );

  const renderHeader = () => (
    <View>
      <View style={styles.searchContainer}>
        <Icon name="search" size={20} color={theme.textSecondary} />
        <TextInput
          style={[styles.searchInput, { color: theme.text }]}
          placeholder="Search markets..."
          placeholderTextColor={theme.textSecondary}
          value={searchQuery}
          onChangeText={setSearchQuery}
        />
        <TouchableOpacity onPress={() => setShowFilters(true)}>
          <Icon name="filter-list" size={24} color={theme.primary} />
        </TouchableOpacity>
      </View>

      <CategoryTabs
        selected={selectedCategory}
        onSelect={setSelectedCategory}
      />

      <View style={styles.stats}>
        <View style={styles.statItem}>
          <Text style={[styles.statValue, { color: theme.primary }]}>
            {markets.length}
          </Text>
          <Text style={[styles.statLabel, { color: theme.textSecondary }]}>
            Active Markets
          </Text>
        </View>
        <View style={styles.statItem}>
          <Text style={[styles.statValue, { color: theme.success }]}>
            $2.4M
          </Text>
          <Text style={[styles.statLabel, { color: theme.textSecondary }]}>
            24h Volume
          </Text>
        </View>
        <View style={styles.statItem}>
          <Text style={[styles.statValue, { color: theme.accent }]}>
            5,234
          </Text>
          <Text style={[styles.statLabel, { color: theme.textSecondary }]}>
            Active Traders
          </Text>
        </View>
      </View>
    </View>
  );

  return (
    <View style={[styles.container, { backgroundColor: theme.background }]}>
      <FlatList
        data={filteredMarkets}
        renderItem={renderMarket}
        keyExtractor={item => item.id}
        ListHeaderComponent={renderHeader}
        refreshControl={
          <RefreshControl
            refreshing={loading}
            onRefresh={refresh}
            tintColor={theme.primary}
          />
        }
        ListEmptyComponent={
          <View style={styles.empty}>
            {loading ? (
              <ActivityIndicator size="large" color={theme.primary} />
            ) : (
              <>
                <Icon name="search-off" size={48} color={theme.textSecondary} />
                <Text style={[styles.emptyText, { color: theme.textSecondary }]}>
                  No markets found
                </Text>
              </>
            )}
          </View>
        }
        contentContainerStyle={styles.listContent}
      />

      <FilterModal
        visible={showFilters}
        onClose={() => setShowFilters(false)}
        onApply={(filters) => {
          // Apply filters
          setShowFilters(false);
        }}
      />
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  searchContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    margin: 16,
    paddingHorizontal: 16,
    height: 48,
    borderRadius: 12,
    backgroundColor: 'rgba(255,255,255,0.05)',
  },
  searchInput: {
    flex: 1,
    marginLeft: 8,
    fontSize: 16,
  },
  stats: {
    flexDirection: 'row',
    justifyContent: 'space-around',
    paddingVertical: 16,
    marginHorizontal: 16,
    borderBottomWidth: 1,
    borderBottomColor: 'rgba(255,255,255,0.1)',
  },
  statItem: {
    alignItems: 'center',
  },
  statValue: {
    fontSize: 20,
    fontWeight: 'bold',
  },
  statLabel: {
    fontSize: 12,
    marginTop: 4,
  },
  marketCard: {
    marginHorizontal: 16,
    marginBottom: 12,
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
});

export default MarketsScreen;