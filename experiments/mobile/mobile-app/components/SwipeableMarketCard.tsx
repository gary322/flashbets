import React, { useRef } from 'react';
import {
  View,
  Text,
  StyleSheet,
  Animated,
  TouchableOpacity,
  Vibration
} from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { Market } from '../types';
import { designTokens } from '../theme';
import { formatVolume, formatTimeRemaining } from '../utils/formatting';

interface SwipeableMarketCardProps {
  market: Market;
  price: number;
  onBuy: () => void;
  onSell: () => void;
  onPress: () => void;
}

const PriceChange: React.FC<{ change: number }> = ({ change }) => {
  const isPositive = change >= 0;
  return (
    <Text style={[
      styles.priceChange,
      { color: isPositive ? designTokens.colors.accent.primary : designTokens.colors.accent.secondary }
    ]}>
      {isPositive ? '+' : ''}{change.toFixed(2)}%
    </Text>
  );
};

const DetailItem: React.FC<{ label: string; value: string }> = ({ label, value }) => (
  <View style={styles.detailItem}>
    <Text style={styles.detailLabel}>{label}</Text>
    <Text style={styles.detailValue}>{value}</Text>
  </View>
);

const SwipeHint: React.FC = () => (
  <View style={styles.swipeHint}>
    <Text style={styles.swipeHintText}>← Swipe to trade →</Text>
  </View>
);

export const SwipeableMarketCard: React.FC<SwipeableMarketCardProps> = ({
  market,
  price,
  onBuy,
  onSell,
  onPress
}) => {
  const swipeableRef = useRef<Swipeable>(null);

  const renderLeftActions = (progress: Animated.AnimatedValue) => {
    const translateX = progress.interpolate({
      inputRange: [0, 1],
      outputRange: [-100, 0]
    });

    return (
      <Animated.View
        style={[
          styles.actionContainer,
          styles.buyAction,
          { transform: [{ translateX }] }
        ]}
      >
        <Text style={styles.actionText}>BUY</Text>
        <Text style={styles.actionSubtext}>Long</Text>
      </Animated.View>
    );
  };

  const renderRightActions = (progress: Animated.AnimatedValue) => {
    const translateX = progress.interpolate({
      inputRange: [0, 1],
      outputRange: [100, 0]
    });

    return (
      <Animated.View
        style={[
          styles.actionContainer,
          styles.sellAction,
          { transform: [{ translateX }] }
        ]}
      >
        <Text style={styles.actionText}>SELL</Text>
        <Text style={styles.actionSubtext}>Short</Text>
      </Animated.View>
    );
  };

  const handleSwipeLeft = () => {
    Vibration.vibrate(10);
    onSell();
    swipeableRef.current?.close();
  };

  const handleSwipeRight = () => {
    Vibration.vibrate(10);
    onBuy();
    swipeableRef.current?.close();
  };

  return (
    <Swipeable
      ref={swipeableRef}
      renderLeftActions={renderLeftActions}
      renderRightActions={renderRightActions}
      onSwipeableOpen={(direction) => {
        if (direction === 'left') {
          handleSwipeLeft();
        } else {
          handleSwipeRight();
        }
      }}
      overshootLeft={false}
      overshootRight={false}
      friction={2}
    >
      <TouchableOpacity
        style={styles.card}
        onPress={onPress}
        activeOpacity={0.95}
      >
        <View style={styles.header}>
          <Text style={styles.marketName} numberOfLines={1}>
            {market.name}
          </Text>
          <View style={styles.priceContainer}>
            <Text style={styles.price}>
              {(price * 100).toFixed(1)}%
            </Text>
            <PriceChange change={market.change24h} />
          </View>
        </View>

        <View style={styles.details}>
          <DetailItem
            label="Volume"
            value={`$${formatVolume(market.volume24h)}`}
          />
          <DetailItem
            label="Liquidity"
            value={`$${formatVolume(market.liquidity)}`}
          />
          <DetailItem
            label="Resolves"
            value={formatTimeRemaining(market.resolutionTime)}
          />
        </View>

        <SwipeHint />
      </TouchableOpacity>
    </Swipeable>
  );
};

const styles = StyleSheet.create({
  card: {
    backgroundColor: designTokens.colors.background.secondary,
    borderRadius: 16,
    padding: 20,
    marginHorizontal: 16,
    marginVertical: 8,
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.05)'
  },

  actionContainer: {
    justifyContent: 'center',
    alignItems: 'center',
    width: 100,
    height: '100%',
    marginHorizontal: 8,
    borderRadius: 16
  },

  buyAction: {
    backgroundColor: designTokens.colors.accent.primary
  },

  sellAction: {
    backgroundColor: designTokens.colors.accent.secondary
  },

  actionText: {
    color: designTokens.colors.text.inverse,
    fontSize: designTokens.typography.sizes.lg,
    fontWeight: designTokens.typography.weights.black
  },

  actionSubtext: {
    color: designTokens.colors.text.inverse,
    fontSize: designTokens.typography.sizes.xs,
    fontWeight: designTokens.typography.weights.semibold,
    opacity: 0.7
  },

  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    marginBottom: designTokens.spacing.md
  },

  marketName: {
    fontSize: designTokens.typography.sizes.base,
    fontWeight: designTokens.typography.weights.semibold,
    color: designTokens.colors.text.primary,
    flex: 1,
    marginRight: designTokens.spacing.md
  },

  priceContainer: {
    alignItems: 'flex-end'
  },

  price: {
    fontSize: designTokens.typography.sizes.xl,
    fontWeight: designTokens.typography.weights.black,
    color: designTokens.colors.text.primary,
    fontFamily: designTokens.typography.fonts.mono
  },

  priceChange: {
    fontSize: designTokens.typography.sizes.sm,
    fontWeight: designTokens.typography.weights.medium,
    marginTop: 2
  },

  details: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginBottom: designTokens.spacing.md
  },

  detailItem: {
    flex: 1,
    alignItems: 'center'
  },

  detailLabel: {
    fontSize: designTokens.typography.sizes.xs,
    color: designTokens.colors.text.tertiary,
    marginBottom: 4
  },

  detailValue: {
    fontSize: designTokens.typography.sizes.sm,
    fontWeight: designTokens.typography.weights.medium,
    color: designTokens.colors.text.secondary
  },

  swipeHint: {
    alignItems: 'center',
    paddingTop: designTokens.spacing.sm
  },

  swipeHintText: {
    fontSize: designTokens.typography.sizes.xs,
    color: designTokens.colors.text.tertiary,
    opacity: 0.5
  }
});