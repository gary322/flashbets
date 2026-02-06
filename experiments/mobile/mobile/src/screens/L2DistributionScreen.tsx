import React, { useState, useRef } from 'react';
import {
  View,
  Text,
  StyleSheet,
  Dimensions,
  TouchableOpacity,
  Alert,
} from 'react-native';
import {
  PanGestureHandler,
  PinchGestureHandler,
  State,
  GestureHandlerRootView,
} from 'react-native-gesture-handler';
import Animated, {
  useAnimatedGestureHandler,
  useAnimatedStyle,
  useSharedValue,
  runOnJS,
  withSpring,
} from 'react-native-reanimated';
import Svg, { Path, Circle, Line, Text as SvgText } from 'react-native-svg';
import HapticFeedback from 'react-native-haptic-feedback';

// Components
import { DistributionChart } from '@components/charts/DistributionChart';
import { ControlPanel } from '@components/L2AMM/ControlPanel';
import { PresetSelector } from '@components/L2AMM/PresetSelector';

// Hooks
import { useL2Distribution } from '@hooks/useL2Distribution';
import { useTheme } from '@hooks/useTheme';

const { width: SCREEN_WIDTH, height: SCREEN_HEIGHT } = Dimensions.get('window');
const CHART_HEIGHT = SCREEN_HEIGHT * 0.4;
const CONTROL_POINT_SIZE = 20;

interface ControlPoint {
  id: string;
  x: number;
  y: number;
  type: 'peak' | 'trough' | 'inflection';
}

const L2DistributionScreen: React.FC = () => {
  const { theme } = useTheme();
  const { 
    distribution, 
    updateDistribution, 
    calculateProbabilities,
    submitDistribution 
  } = useL2Distribution();

  const [controlPoints, setControlPoints] = useState<ControlPoint[]>([
    { id: '1', x: 0.2, y: 0.3, type: 'peak' },
    { id: '2', x: 0.5, y: 0.8, type: 'peak' },
    { id: '3', x: 0.8, y: 0.4, type: 'peak' },
  ]);

  const [selectedPoint, setSelectedPoint] = useState<string | null>(null);
  const [isDrawing, setIsDrawing] = useState(false);

  // Animated values for gestures
  const translateX = useSharedValue(0);
  const translateY = useSharedValue(0);
  const scale = useSharedValue(1);

  // Generate smooth curve path from control points
  const generatePath = (): string => {
    if (controlPoints.length < 2) return '';

    let path = `M ${controlPoints[0].x * SCREEN_WIDTH} ${
      (1 - controlPoints[0].y) * CHART_HEIGHT
    }`;

    // Use Catmull-Rom splines for smooth curves
    for (let i = 1; i < controlPoints.length; i++) {
      const prev = controlPoints[i - 1];
      const curr = controlPoints[i];
      const next = controlPoints[i + 1] || curr;

      const cp1x = prev.x + (curr.x - prev.x) * 0.3;
      const cp1y = prev.y + (curr.y - prev.y) * 0.3;
      const cp2x = curr.x - (next.x - prev.x) * 0.3;
      const cp2y = curr.y - (next.y - prev.y) * 0.3;

      path += ` C ${cp1x * SCREEN_WIDTH} ${(1 - cp1y) * CHART_HEIGHT}, ${
        cp2x * SCREEN_WIDTH
      } ${(1 - cp2y) * CHART_HEIGHT}, ${curr.x * SCREEN_WIDTH} ${
        (1 - curr.y) * CHART_HEIGHT
      }`;
    }

    return path;
  };

  // Pan gesture for moving control points
  const panGestureHandler = useAnimatedGestureHandler({
    onStart: (_, ctx: any) => {
      ctx.startX = translateX.value;
      ctx.startY = translateY.value;
    },
    onActive: (event, ctx) => {
      if (selectedPoint) {
        translateX.value = ctx.startX + event.translationX;
        translateY.value = ctx.startY + event.translationY;
        
        runOnJS(updateControlPoint)(
          selectedPoint,
          event.absoluteX / SCREEN_WIDTH,
          1 - event.absoluteY / CHART_HEIGHT
        );
      }
    },
    onEnd: () => {
      translateX.value = withSpring(0);
      translateY.value = withSpring(0);
      runOnJS(HapticFeedback.impact)(HapticFeedback.ImpactFeedbackStyle.Light);
    },
  });

  // Pinch gesture for scaling distribution
  const pinchGestureHandler = useAnimatedGestureHandler({
    onActive: (event) => {
      scale.value = event.scale;
    },
    onEnd: () => {
      scale.value = withSpring(1);
      runOnJS(HapticFeedback.impact)(HapticFeedback.ImpactFeedbackStyle.Medium);
    },
  });

  const updateControlPoint = (id: string, x: number, y: number) => {
    setControlPoints(prev =>
      prev.map(point =>
        point.id === id
          ? { ...point, x: Math.max(0, Math.min(1, x)), y: Math.max(0, Math.min(1, y)) }
          : point
      )
    );
    
    // Update distribution in real-time
    const probs = calculateProbabilities(controlPoints);
    updateDistribution(probs);
  };

  const addControlPoint = (x: number, y: number) => {
    const newPoint: ControlPoint = {
      id: Date.now().toString(),
      x: x / SCREEN_WIDTH,
      y: 1 - y / CHART_HEIGHT,
      type: 'peak',
    };
    
    setControlPoints(prev => [...prev, newPoint].sort((a, b) => a.x - b.x));
    HapticFeedback.impact(HapticFeedback.ImpactFeedbackStyle.Heavy);
  };

  const removeControlPoint = (id: string) => {
    if (controlPoints.length <= 2) {
      Alert.alert('Minimum Points', 'You need at least 2 control points');
      return;
    }
    
    setControlPoints(prev => prev.filter(point => point.id !== id));
    HapticFeedback.impact(HapticFeedback.ImpactFeedbackStyle.Medium);
  };

  const handlePresetSelect = (preset: string) => {
    switch (preset) {
      case 'normal':
        setControlPoints([
          { id: '1', x: 0.2, y: 0.2, type: 'trough' },
          { id: '2', x: 0.5, y: 0.9, type: 'peak' },
          { id: '3', x: 0.8, y: 0.2, type: 'trough' },
        ]);
        break;
      case 'bimodal':
        setControlPoints([
          { id: '1', x: 0.3, y: 0.8, type: 'peak' },
          { id: '2', x: 0.5, y: 0.4, type: 'trough' },
          { id: '3', x: 0.7, y: 0.8, type: 'peak' },
        ]);
        break;
      case 'skewed':
        setControlPoints([
          { id: '1', x: 0.2, y: 0.9, type: 'peak' },
          { id: '2', x: 0.4, y: 0.6, type: 'inflection' },
          { id: '3', x: 0.8, y: 0.1, type: 'trough' },
        ]);
        break;
    }
    HapticFeedback.selection();
  };

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  return (
    <View style={[styles.container, { backgroundColor: theme.background }]}>
      <View style={styles.header}>
        <Text style={[styles.title, { color: theme.text }]}>
          L2 Distribution Editor
        </Text>
        <TouchableOpacity
          style={[styles.submitButton, { backgroundColor: theme.primary }]}
          onPress={() => submitDistribution(controlPoints)}>
          <Text style={styles.submitText}>Submit</Text>
        </TouchableOpacity>
      </View>

      <PresetSelector onSelect={handlePresetSelect} />

      <GestureHandlerRootView style={styles.chartContainer}>
        <PinchGestureHandler onGestureEvent={pinchGestureHandler}>
          <Animated.View style={[styles.chart, animatedStyle]}>
            <PanGestureHandler onGestureEvent={panGestureHandler}>
              <Animated.View style={StyleSheet.absoluteFillObject}>
                <Svg
                  width={SCREEN_WIDTH}
                  height={CHART_HEIGHT}
                  onStartShouldSetResponder={() => true}
                  onResponderGrant={(e) => {
                    if (!selectedPoint && isDrawing) {
                      addControlPoint(e.nativeEvent.locationX, e.nativeEvent.locationY);
                    }
                  }}>
                  {/* Grid lines */}
                  {[0.2, 0.4, 0.6, 0.8].map(y => (
                    <Line
                      key={y}
                      x1={0}
                      x2={SCREEN_WIDTH}
                      y1={y * CHART_HEIGHT}
                      y2={y * CHART_HEIGHT}
                      stroke={theme.border}
                      strokeWidth={0.5}
                      strokeDasharray="5,5"
                    />
                  ))}

                  {/* Distribution curve */}
                  <Path
                    d={generatePath()}
                    stroke={theme.primary}
                    strokeWidth={3}
                    fill="none"
                  />

                  {/* Area under curve */}
                  <Path
                    d={`${generatePath()} L ${SCREEN_WIDTH} ${CHART_HEIGHT} L 0 ${CHART_HEIGHT} Z`}
                    fill={`${theme.primary}20`}
                  />

                  {/* Control points */}
                  {controlPoints.map(point => (
                    <Circle
                      key={point.id}
                      cx={point.x * SCREEN_WIDTH}
                      cy={(1 - point.y) * CHART_HEIGHT}
                      r={CONTROL_POINT_SIZE / 2}
                      fill={selectedPoint === point.id ? theme.accent : theme.primary}
                      onPress={() => setSelectedPoint(point.id)}
                      onLongPress={() => removeControlPoint(point.id)}
                    />
                  ))}

                  {/* Probability labels */}
                  {controlPoints.map((point, index) => (
                    <SvgText
                      key={`label-${point.id}`}
                      x={point.x * SCREEN_WIDTH}
                      y={(1 - point.y) * CHART_HEIGHT - 15}
                      fill={theme.textSecondary}
                      fontSize={12}
                      textAnchor="middle">
                      {(point.y * 100).toFixed(0)}%
                    </SvgText>
                  ))}
                </Svg>
              </Animated.View>
            </PanGestureHandler>
          </Animated.View>
        </PinchGestureHandler>
      </GestureHandlerRootView>

      <ControlPanel
        isDrawing={isDrawing}
        onToggleDrawing={() => setIsDrawing(!isDrawing)}
        distribution={distribution}
      />

      <DistributionChart
        data={distribution}
        height={100}
        style={styles.miniChart}
      />
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: 16,
  },
  title: {
    fontSize: 24,
    fontWeight: 'bold',
  },
  submitButton: {
    paddingHorizontal: 20,
    paddingVertical: 10,
    borderRadius: 8,
  },
  submitText: {
    color: 'white',
    fontWeight: '600',
  },
  chartContainer: {
    flex: 1,
    marginVertical: 10,
  },
  chart: {
    width: SCREEN_WIDTH,
    height: CHART_HEIGHT,
  },
  miniChart: {
    marginHorizontal: 16,
    marginBottom: 16,
  },
});

export default L2DistributionScreen;