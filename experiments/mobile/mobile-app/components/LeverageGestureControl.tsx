import React, { useState, useRef } from 'react';
import {
  View,
  Text,
  PanResponder,
  Animated,
  Vibration,
  StyleSheet,
  Dimensions
} from 'react-native';
import { designTokens } from '../theme';

interface LeverageGestureControlProps {
  initialValue: number;
  onChange: (value: number) => void;
  max: number;
}

interface HapticFeedback {
  trigger: (type: 'light' | 'medium' | 'heavy') => void;
}

// Mock haptic feedback for development - in production, use react-native-haptic-feedback
const Haptics: HapticFeedback = {
  trigger: (type: 'light' | 'medium' | 'heavy') => {
    const durations = {
      light: 10,
      medium: 20,
      heavy: 50
    };
    Vibration.vibrate(durations[type]);
  }
};

const FadeInView: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const fadeAnim = useRef(new Animated.Value(0)).current;

  React.useEffect(() => {
    Animated.timing(fadeAnim, {
      toValue: 1,
      duration: 300,
      useNativeDriver: true
    }).start();
  }, [fadeAnim]);

  return (
    <Animated.View style={{ opacity: fadeAnim }}>
      {children}
    </Animated.View>
  );
};

export const LeverageGestureControl: React.FC<LeverageGestureControlProps> = ({
  initialValue,
  onChange,
  max
}) => {
  const [leverage, setLeverage] = useState(initialValue);
  const translateY = useRef(new Animated.Value(0)).current;
  const scale = useRef(new Animated.Value(1)).current;
  const lastHapticThreshold = useRef(0);

  const hapticFeedback = (intensity: 'light' | 'medium' | 'heavy') => {
    Haptics.trigger(intensity);
  };

  const panResponder = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,
      onMoveShouldSetPanResponder: () => true,

      onPanResponderGrant: () => {
        Animated.spring(scale, {
          toValue: 1.2,
          useNativeDriver: true
        }).start();
        hapticFeedback('light');
      },

      onPanResponderMove: (_, gestureState) => {
        const delta = -gestureState.dy / 5; // Sensitivity
        const newValue = Math.max(1, Math.min(max, initialValue + delta));

        // Haptic feedback at thresholds
        const currentThreshold = Math.floor(newValue / 10);
        if (currentThreshold !== lastHapticThreshold.current) {
          hapticFeedback('light');
          lastHapticThreshold.current = currentThreshold;
        }

        // Strong feedback at danger zones
        if (newValue >= 100 && leverage < 100) {
          hapticFeedback('medium');
        }

        if (newValue >= 300 && leverage < 300) {
          hapticFeedback('heavy');
          Vibration.vibrate(50);
        }

        setLeverage(Math.round(newValue));

        Animated.event(
          [null, { dy: translateY }],
          { useNativeDriver: false }
        )(null, gestureState);
      },

      onPanResponderRelease: () => {
        Animated.parallel([
          Animated.spring(translateY, {
            toValue: 0,
            useNativeDriver: true
          }),
          Animated.spring(scale, {
            toValue: 1,
            useNativeDriver: true
          })
        ]).start();

        onChange(leverage);
        hapticFeedback('light');
      }
    })
  ).current;

  const getColorForLeverage = (value: number) => {
    if (value >= 300) return designTokens.colors.status.liquidation;
    if (value >= 100) return designTokens.colors.accent.warning;
    if (value >= 50) return designTokens.colors.accent.leverage;
    return designTokens.colors.accent.primary;
  };

  const getLiquidationBuffer = () => {
    return (100 / leverage).toFixed(2);
  };

  return (
    <View style={styles.container}>
      <Animated.View
        style={[
          styles.leverageDisplay,
          {
            transform: [
              { translateY },
              { scale }
            ]
          }
        ]}
        {...panResponder.panHandlers}
      >
        <Text
          style={[
            styles.leverageValue,
            { color: getColorForLeverage(leverage) }
          ]}
        >
          {leverage}x
        </Text>
        <Text style={styles.leverageLabel}>
          Swipe up/down to adjust
        </Text>
      </Animated.View>

      {leverage >= 100 && (
        <FadeInView>
          <View style={styles.warning}>
            <Text style={styles.warningText}>
              ⚠️ High leverage - {getLiquidationBuffer()}% liquidation buffer
            </Text>
          </View>
        </FadeInView>
      )}

      <View style={styles.leverageBar}>
        <View 
          style={[
            styles.leverageFill,
            { 
              height: `${(leverage / max) * 100}%`,
              backgroundColor: getColorForLeverage(leverage)
            }
          ]}
        />
      </View>

      <View style={styles.presetContainer}>
        {[10, 50, 100, 200].map(preset => (
          <Text
            key={preset}
            style={[
              styles.presetLabel,
              leverage >= preset && styles.presetLabelActive
            ]}
          >
            {preset}x
          </Text>
        ))}
      </View>
    </View>
  );
};

const { height: screenHeight } = Dimensions.get('window');

const styles = StyleSheet.create({
  container: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: designTokens.spacing.xl
  },

  leverageDisplay: {
    alignItems: 'center',
    padding: designTokens.spacing.xl,
    backgroundColor: designTokens.colors.background.secondary,
    borderRadius: 24,
    borderWidth: 2,
    borderColor: 'rgba(255, 255, 255, 0.1)',
    elevation: 5,
    shadowColor: '#000',
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.3,
    shadowRadius: 8
  },

  leverageValue: {
    fontSize: designTokens.typography.sizes['4xl'],
    fontWeight: designTokens.typography.weights.black,
    fontFamily: designTokens.typography.fonts.mono
  },

  leverageLabel: {
    fontSize: designTokens.typography.sizes.sm,
    color: designTokens.colors.text.secondary,
    marginTop: designTokens.spacing.sm
  },

  warning: {
    position: 'absolute',
    bottom: 100,
    left: 20,
    right: 20,
    backgroundColor: 'rgba(220, 38, 38, 0.1)',
    borderColor: 'rgba(220, 38, 38, 0.3)',
    borderWidth: 1,
    borderRadius: 8,
    padding: designTokens.spacing.md
  },

  warningText: {
    color: designTokens.colors.status.error,
    fontSize: designTokens.typography.sizes.sm,
    textAlign: 'center',
    fontWeight: designTokens.typography.weights.medium
  },

  leverageBar: {
    position: 'absolute',
    right: 20,
    top: 100,
    bottom: 100,
    width: 4,
    backgroundColor: designTokens.colors.background.tertiary,
    borderRadius: 2,
    overflow: 'hidden'
  },

  leverageFill: {
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
    borderRadius: 2,
    transition: 'height 0.3s ease'
  },

  presetContainer: {
    position: 'absolute',
    right: 30,
    top: 100,
    bottom: 100,
    justifyContent: 'space-between'
  },

  presetLabel: {
    fontSize: designTokens.typography.sizes.xs,
    color: designTokens.colors.text.tertiary,
    textAlign: 'right'
  },

  presetLabelActive: {
    color: designTokens.colors.text.primary,
    fontWeight: designTokens.typography.weights.bold
  }
});