import React, { useState, useRef } from 'react';
import {
  View,
  Text,
  StyleSheet,
  ScrollView,
  TouchableOpacity,
  Dimensions,
  Alert,
} from 'react-native';
import DraggableFlatList, {
  ScaleDecorator,
  RenderItemParams,
} from 'react-native-draggable-flatlist';
import { Modalize } from 'react-native-modalize';
import Icon from 'react-native-vector-icons/MaterialIcons';
import HapticFeedback from 'react-native-haptic-feedback';

// Components
import { ChainStepCard } from '@components/chains/ChainStepCard';
import { MarketSelector } from '@components/chains/MarketSelector';
import { ChainPreview } from '@components/chains/ChainPreview';
import { LeverageCalculator } from '@components/chains/LeverageCalculator';

// Hooks
import { useChainBuilder } from '@hooks/useChainBuilder';
import { useTheme } from '@hooks/useTheme';
import { useWallet } from '@hooks/useWallet';

// Types
import { ChainStep, Market } from '@types';

const { height: SCREEN_HEIGHT } = Dimensions.get('window');

const ChainBuilderScreen: React.FC = () => {
  const { theme } = useTheme();
  const { connected } = useWallet();
  const {
    chain,
    addStep,
    removeStep,
    updateStep,
    reorderSteps,
    calculateTotalLeverage,
    validateChain,
    executeChain,
  } = useChainBuilder();

  const [selectedMarket, setSelectedMarket] = useState<Market | null>(null);
  const modalizeRef = useRef<Modalize>(null);

  const handleAddStep = () => {
    if (!connected) {
      Alert.alert('Wallet Required', 'Please connect your wallet first');
      return;
    }

    modalizeRef.current?.open();
  };

  const handleMarketSelect = (market: Market) => {
    const newStep: ChainStep = {
      id: Date.now().toString(),
      marketId: market.id,
      marketName: market.name,
      outcome: 0,
      size: 100,
      leverage: 1,
      conditional: 'previous_wins',
    };

    addStep(newStep);
    modalizeRef.current?.close();
    HapticFeedback.impact(HapticFeedback.ImpactFeedbackStyle.Medium);
  };

  const handleExecuteChain = async () => {
    const validation = validateChain();
    if (!validation.valid) {
      Alert.alert('Invalid Chain', validation.error || 'Please check your chain configuration');
      return;
    }

    try {
      await executeChain();
      Alert.alert('Success', 'Chain executed successfully!');
    } catch (error) {
      Alert.alert('Execution Failed', error.message);
    }
  };

  const renderChainStep = ({ item, drag, isActive }: RenderItemParams<ChainStep>) => (
    <ScaleDecorator>
      <TouchableOpacity
        onLongPress={drag}
        disabled={isActive}
        delayLongPress={150}>
        <ChainStepCard
          step={item}
          onUpdate={(updates) => updateStep(item.id, updates)}
          onRemove={() => removeStep(item.id)}
          isActive={isActive}
        />
      </TouchableOpacity>
    </ScaleDecorator>
  );

  const totalLeverage = calculateTotalLeverage();
  const maxLeverage = chain.length * 60; // 60x per step max

  return (
    <View style={[styles.container, { backgroundColor: theme.background }]}>
      <ScrollView style={styles.content} showsVerticalScrollIndicator={false}>
        <View style={styles.header}>
          <Text style={[styles.title, { color: theme.text }]}>
            Chain Builder
          </Text>
          <Text style={[styles.subtitle, { color: theme.textSecondary }]}>
            Create conditional execution chains for +180% leverage
          </Text>
        </View>

        <LeverageCalculator
          currentLeverage={totalLeverage}
          maxLeverage={maxLeverage}
          steps={chain.length}
        />

        {chain.length > 0 ? (
          <>
            <DraggableFlatList
              data={chain}
              onDragEnd={({ data }) => reorderSteps(data)}
              keyExtractor={(item) => item.id}
              renderItem={renderChainStep}
              scrollEnabled={false}
              contentContainerStyle={styles.chainList}
            />

            <ChainPreview chain={chain} />
          </>
        ) : (
          <View style={styles.emptyState}>
            <Icon name="link" size={64} color={theme.textSecondary} />
            <Text style={[styles.emptyText, { color: theme.textSecondary }]}>
              No chain steps yet
            </Text>
            <Text style={[styles.emptySubtext, { color: theme.textSecondary }]}>
              Add your first step to start building
            </Text>
          </View>
        )}
      </ScrollView>

      <View style={styles.footer}>
        <TouchableOpacity
          style={[styles.addButton, { backgroundColor: theme.primary }]}
          onPress={handleAddStep}>
          <Icon name="add" size={24} color="white" />
          <Text style={styles.addButtonText}>Add Step</Text>
        </TouchableOpacity>

        {chain.length >= 2 && (
          <TouchableOpacity
            style={[
              styles.executeButton,
              { backgroundColor: theme.success },
            ]}
            onPress={handleExecuteChain}>
            <Icon name="flash-on" size={24} color="white" />
            <Text style={styles.executeButtonText}>Execute Chain</Text>
          </TouchableOpacity>
        )}
      </View>

      <Modalize
        ref={modalizeRef}
        snapPoint={SCREEN_HEIGHT * 0.7}
        modalHeight={SCREEN_HEIGHT * 0.9}
        handleStyle={[styles.modalHandle, { backgroundColor: theme.border }]}
        modalStyle={[styles.modal, { backgroundColor: theme.card }]}>
        <MarketSelector
          onSelect={handleMarketSelect}
          excludeMarkets={chain.map(s => s.marketId)}
        />
      </Modalize>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  content: {
    flex: 1,
  },
  header: {
    padding: 20,
  },
  title: {
    fontSize: 28,
    fontWeight: 'bold',
    marginBottom: 8,
  },
  subtitle: {
    fontSize: 16,
  },
  chainList: {
    paddingHorizontal: 16,
  },
  emptyState: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    paddingVertical: 100,
  },
  emptyText: {
    fontSize: 18,
    fontWeight: '600',
    marginTop: 16,
  },
  emptySubtext: {
    fontSize: 14,
    marginTop: 8,
  },
  footer: {
    flexDirection: 'row',
    padding: 16,
    gap: 12,
    borderTopWidth: 1,
    borderTopColor: 'rgba(255,255,255,0.1)',
  },
  addButton: {
    flex: 1,
    flexDirection: 'row',
    justifyContent: 'center',
    alignItems: 'center',
    paddingVertical: 16,
    borderRadius: 12,
    gap: 8,
  },
  addButtonText: {
    color: 'white',
    fontSize: 16,
    fontWeight: '600',
  },
  executeButton: {
    flex: 1,
    flexDirection: 'row',
    justifyContent: 'center',
    alignItems: 'center',
    paddingVertical: 16,
    borderRadius: 12,
    gap: 8,
  },
  executeButtonText: {
    color: 'white',
    fontSize: 16,
    fontWeight: '600',
  },
  modal: {
    borderTopLeftRadius: 20,
    borderTopRightRadius: 20,
  },
  modalHandle: {
    width: 40,
    height: 4,
    marginTop: 12,
    alignSelf: 'center',
    borderRadius: 2,
  },
});

export default ChainBuilderScreen;