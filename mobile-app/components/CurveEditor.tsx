import React, { useRef, useState } from 'react';
import {
  View,
  StyleSheet,
  PanResponder,
  Dimensions,
  Text,
  Vibration
} from 'react-native';
import Svg, { Path, Circle, Line, Text as SvgText } from 'react-native-svg';
import { designTokens } from '../theme';

const { width: screenWidth } = Dimensions.get('window');
const CHART_WIDTH = screenWidth - 40;
const CHART_HEIGHT = 300;

interface CurveEditorProps {
  initialMean: number;
  initialVariance: number;
  minValue: number;
  maxValue: number;
  onChange: (mean: number, variance: number) => void;
}

export const CurveEditor: React.FC<CurveEditorProps> = ({
  initialMean,
  initialVariance,
  minValue,
  maxValue,
  onChange
}) => {
  const [mean, setMean] = useState(initialMean);
  const [variance, setVariance] = useState(initialVariance);
  const [isPinching, setIsPinching] = useState(false);
  const [initialDistance, setInitialDistance] = useState(0);

  // Generate normal distribution curve points
  const generateCurvePoints = () => {
    const points: { x: number; y: number }[] = [];
    const steps = 100;
    const range = maxValue - minValue;

    for (let i = 0; i <= steps; i++) {
      const x = minValue + (range * i / steps);
      const exponent = -Math.pow(x - mean, 2) / (2 * variance);
      const y = Math.exp(exponent) / Math.sqrt(2 * Math.PI * variance);

      points.push({
        x: ((x - minValue) / range) * CHART_WIDTH,
        y: CHART_HEIGHT - (y * CHART_HEIGHT * Math.sqrt(variance) * 2)
      });
    }

    return points;
  };

  const curvePoints = generateCurvePoints();
  const pathData = curvePoints
    .map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`)
    .join(' ');

  // Pan responder for dragging mean
  const meanPanResponder = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: () => true,

      onPanResponderGrant: () => {
        Vibration.vibrate(10);
      },

      onPanResponderMove: (_, gestureState) => {
        const range = maxValue - minValue;
        const newMean = Math.max(
          minValue,
          Math.min(
            maxValue,
            mean + (gestureState.dx / CHART_WIDTH) * range
          )
        );
        setMean(newMean);
      },

      onPanResponderRelease: () => {
        onChange(mean, variance);
      }
    })
  ).current;

  // Gesture responder for pinch-to-zoom variance
  const varianceResponder = useRef(
    PanResponder.create({
      onStartShouldSetPanResponder: (_, gestureState) => {
        return gestureState.numberActiveTouches === 2;
      },

      onPanResponderGrant: (evt) => {
        const touches = evt.nativeEvent.touches;
        if (touches.length === 2) {
          const distance = Math.sqrt(
            Math.pow(touches[0].pageX - touches[1].pageX, 2) +
            Math.pow(touches[0].pageY - touches[1].pageY, 2)
          );
          setInitialDistance(distance);
          setIsPinching(true);
          Vibration.vibrate(10);
        }
      },

      onPanResponderMove: (evt) => {
        const touches = evt.nativeEvent.touches;
        if (touches.length === 2 && isPinching) {
          const distance = Math.sqrt(
            Math.pow(touches[0].pageX - touches[1].pageX, 2) +
            Math.pow(touches[0].pageY - touches[1].pageY, 2)
          );

          const scale = distance / initialDistance;
          const newVariance = Math.max(0.1, Math.min(10, variance * scale));
          setVariance(newVariance);
        }
      },

      onPanResponderRelease: () => {
        setIsPinching(false);
        onChange(mean, variance);
      }
    })
  ).current;

  const meanX = ((mean - minValue) / (maxValue - minValue)) * CHART_WIDTH;

  return (
    <View style={styles.container}>
      <Text style={styles.title}>Drag to adjust prediction curve</Text>

      <View 
        style={styles.chartContainer}
        {...varianceResponder.panHandlers}
      >
        <Svg width={CHART_WIDTH} height={CHART_HEIGHT}>
          {/* Grid lines */}
          {[0, 0.25, 0.5, 0.75, 1].map(ratio => (
            <React.Fragment key={ratio}>
              <Line
                x1={ratio * CHART_WIDTH}
                y1={0}
                x2={ratio * CHART_WIDTH}
                y2={CHART_HEIGHT}
                stroke={designTokens.colors.background.tertiary}
                strokeWidth="1"
                strokeDasharray="5,5"
              />
              <SvgText
                x={ratio * CHART_WIDTH}
                y={CHART_HEIGHT - 5}
                fontSize="10"
                fill={designTokens.colors.text.tertiary}
                textAnchor="middle"
              >
                {(minValue + (maxValue - minValue) * ratio).toFixed(0)}
              </SvgText>
            </React.Fragment>
          ))}

          {/* Distribution curve */}
          <Path
            d={pathData}
            fill="none"
            stroke={designTokens.colors.accent.primary}
            strokeWidth="3"
          />

          {/* Mean indicator */}
          <View {...meanPanResponder.panHandlers}>
            <Line
              x1={meanX}
              y1={0}
              x2={meanX}
              y2={CHART_HEIGHT}
              stroke={designTokens.colors.accent.leverage}
              strokeWidth="2"
            />
            <Circle
              cx={meanX}
              cy={CHART_HEIGHT / 2}
              r="10"
              fill={designTokens.colors.accent.leverage}
            />
          </View>
        </Svg>
      </View>

      <View style={styles.statsContainer}>
        <View style={styles.statBox}>
          <Text style={styles.statLabel}>Mean</Text>
          <Text style={styles.statValue}>{mean.toFixed(1)}</Text>
        </View>
        <View style={styles.statBox}>
          <Text style={styles.statLabel}>Std Dev</Text>
          <Text style={styles.statValue}>{Math.sqrt(variance).toFixed(2)}</Text>
        </View>
      </View>

      <Text style={styles.hint}>
        Drag line to move • Pinch to adjust spread
      </Text>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    padding: 20
  },

  title: {
    fontSize: designTokens.typography.sizes.lg,
    fontWeight: designTokens.typography.weights.semibold,
    color: designTokens.colors.text.primary,
    marginBottom: 20
  },

  chartContainer: {
    backgroundColor: designTokens.colors.background.secondary,
    borderRadius: 12,
    padding: 20,
    marginBottom: 20
  },

  statsContainer: {
    flexDirection: 'row',
    justifyContent: 'space-around',
    marginBottom: 10
  },

  statBox: {
    alignItems: 'center'
  },

  statLabel: {
    fontSize: designTokens.typography.sizes.sm,
    color: designTokens.colors.text.secondary,
    marginBottom: 4
  },

  statValue: {
    fontSize: designTokens.typography.sizes.xl,
    fontWeight: designTokens.typography.weights.bold,
    color: designTokens.colors.text.primary,
    fontFamily: designTokens.typography.fonts.mono
  },

  hint: {
    fontSize: designTokens.typography.sizes.xs,
    color: designTokens.colors.text.tertiary,
    textAlign: 'center'
  }
});