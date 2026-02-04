import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { VerseNode } from '../components/verse/VerseTree';
import { VerseData } from '../components/verse/VerseCard';

interface VerseContextState {
  // Verse data
  rootVerses: VerseNode[];
  allVerses: Map<string, VerseData>;
  selectedVerseIds: Set<string>;
  expandedVerseIds: Set<string>;
  
  // Loading states
  isLoading: boolean;
  error: string | null;
  
  // Actions
  loadVerses: () => Promise<void>;
  selectVerse: (verseId: string) => void;
  deselectVerse: (verseId: string) => void;
  toggleVerseExpansion: (verseId: string) => void;
  getVersesForMarket: (marketId: string) => VerseData[];
  calculateTotalMultiplier: () => number;
  searchVerses: (query: string) => VerseData[];
}

const VerseContext = createContext<VerseContextState | undefined>(undefined);

export function useVerseContext() {
  const context = useContext(VerseContext);
  if (!context) {
    throw new Error('useVerseContext must be used within a VerseProvider');
  }
  return context;
}

interface VerseProviderProps {
  children: React.ReactNode;
}

export function VerseProvider({ children }: VerseProviderProps) {
  const [rootVerses, setRootVerses] = useState<VerseNode[]>([]);
  const [allVerses, setAllVerses] = useState<Map<string, VerseData>>(new Map());
  const [selectedVerseIds, setSelectedVerseIds] = useState<Set<string>>(new Set());
  const [expandedVerseIds, setExpandedVerseIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load verses from API
  const loadVerses = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      const response = await fetch('/api/verses');
      if (!response.ok) {
        throw new Error('Failed to load verses');
      }
      
      const data = await response.json();
      
      // Build verse tree structure
      const verses = buildVerseTree(data.verses || []);
      setRootVerses(verses.roots);
      setAllVerses(verses.map);
      
      // Auto-expand first level
      const firstLevelIds = verses.roots.map(v => v.id);
      setExpandedVerseIds(new Set(firstLevelIds));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load verses');
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Select a verse
  const selectVerse = useCallback((verseId: string) => {
    setSelectedVerseIds(prev => {
      const next = new Set(prev);
      if (next.has(verseId)) {
        next.delete(verseId);
      } else {
        // Check if we can add more verses (max 3 for safety)
        if (next.size < 3) {
          next.add(verseId);
        }
      }
      return next;
    });
  }, []);

  // Deselect a verse
  const deselectVerse = useCallback((verseId: string) => {
    setSelectedVerseIds(prev => {
      const next = new Set(prev);
      next.delete(verseId);
      return next;
    });
  }, []);

  // Toggle verse expansion
  const toggleVerseExpansion = useCallback((verseId: string) => {
    setExpandedVerseIds(prev => {
      const next = new Set(prev);
      if (next.has(verseId)) {
        next.delete(verseId);
      } else {
        next.add(verseId);
      }
      return next;
    });
  }, []);

  // Get verses for a specific market
  const getVersesForMarket = useCallback((marketId: string): VerseData[] => {
    // This would be enhanced with real market-verse matching logic
    const verses: VerseData[] = [];
    
    for (const verse of allVerses.values()) {
      // Simple category matching for now
      if (verse.category && marketId.toLowerCase().includes(verse.category.toLowerCase())) {
        verses.push(verse);
      }
    }
    
    return verses;
  }, [allVerses]);

  // Calculate total multiplier from selected verses
  const calculateTotalMultiplier = useCallback(() => {
    let multiplier = 1;
    
    for (const verseId of selectedVerseIds) {
      const verse = allVerses.get(verseId);
      if (verse) {
        multiplier *= verse.multiplier;
      }
    }
    
    // Cap at 100x total
    return Math.min(multiplier, 100);
  }, [selectedVerseIds, allVerses]);

  // Search verses
  const searchVerses = useCallback((query: string): VerseData[] => {
    const results: VerseData[] = [];
    const searchTerm = query.toLowerCase();
    
    for (const verse of allVerses.values()) {
      if (
        verse.name.toLowerCase().includes(searchTerm) ||
        verse.description.toLowerCase().includes(searchTerm) ||
        verse.category.toLowerCase().includes(searchTerm)
      ) {
        results.push(verse);
      }
    }
    
    return results;
  }, [allVerses]);

  // Load verses on mount
  useEffect(() => {
    loadVerses();
  }, [loadVerses]);

  const value: VerseContextState = {
    rootVerses,
    allVerses,
    selectedVerseIds,
    expandedVerseIds,
    isLoading,
    error,
    loadVerses,
    selectVerse,
    deselectVerse,
    toggleVerseExpansion,
    getVersesForMarket,
    calculateTotalMultiplier,
    searchVerses,
  };

  return (
    <VerseContext.Provider value={value}>
      {children}
    </VerseContext.Provider>
  );
}

// Helper function to build verse tree from flat data
function buildVerseTree(versesData: any[]): { 
  roots: VerseNode[]; 
  map: Map<string, VerseData> 
} {
  const map = new Map<string, VerseData>();
  const nodeMap = new Map<string, VerseNode>();
  const roots: VerseNode[] = [];

  // First pass: create all verses
  for (const data of versesData) {
    const verseData: VerseData = {
      id: data.id,
      name: data.name,
      description: data.description || '',
      multiplier: data.multiplier || 1,
      marketCount: data.market_count || 0,
      totalVolume: data.total_volume || 0,
      participants: data.participants || 0,
      risk_tier: data.risk_tier || 'Medium',
      category: data.category || 'General',
    };
    
    map.set(data.id, verseData);
    
    const node: VerseNode = {
      id: data.id,
      name: data.name,
      type: data.level === 1 ? 'root' : 
            data.level === 2 ? 'category' : 
            data.level === 3 ? 'subcategory' : 'market',
      children: [],
      marketCount: data.market_count,
      multiplier: data.multiplier,
      active: true,
    };
    
    nodeMap.set(data.id, node);
  }

  // Second pass: build hierarchy
  for (const data of versesData) {
    const node = nodeMap.get(data.id)!;
    
    if (data.parent_id) {
      const parent = nodeMap.get(data.parent_id);
      if (parent) {
        parent.children!.push(node);
      }
    } else {
      roots.push(node);
    }
  }

  return { roots, map };
}
