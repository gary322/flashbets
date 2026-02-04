import { useState, useEffect, useCallback } from 'react';
import { VerseData } from '../components/verse/VerseCard';

interface UseVersesOptions {
  marketId?: string;
  category?: string;
  autoLoad?: boolean;
}

interface UseVersesResult {
  verses: VerseData[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
  searchVerses: (query: string) => VerseData[];
}

export function useVerses(options: UseVersesOptions = {}): UseVersesResult {
  const { marketId, category, autoLoad = true } = options;
  const [verses, setVerses] = useState<VerseData[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchVerses = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      let url = '/api/verses';
      const params = new URLSearchParams();
      
      if (marketId) {
        params.append('market_id', marketId);
      }
      if (category) {
        params.append('category', category);
      }
      
      if (params.toString()) {
        url += `?${params.toString()}`;
      }

      const response = await fetch(url);
      if (!response.ok) {
        throw new Error('Failed to fetch verses');
      }

      const data = await response.json();
      const versesData = (data.verses || []).map(transformVerseData);
      setVerses(versesData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load verses');
    } finally {
      setLoading(false);
    }
  }, [marketId, category]);

  const searchVerses = useCallback((query: string): VerseData[] => {
    const searchTerm = query.toLowerCase();
    return verses.filter(verse => 
      verse.name.toLowerCase().includes(searchTerm) ||
      verse.description.toLowerCase().includes(searchTerm) ||
      verse.category.toLowerCase().includes(searchTerm)
    );
  }, [verses]);

  useEffect(() => {
    if (autoLoad) {
      fetchVerses();
    }
  }, [fetchVerses, autoLoad]);

  return {
    verses,
    loading,
    error,
    refetch: fetchVerses,
    searchVerses,
  };
}

// Transform API data to component format
function transformVerseData(apiData: any): VerseData {
  return {
    id: apiData.id,
    name: apiData.name,
    description: apiData.description || '',
    multiplier: apiData.multiplier || 1,
    marketCount: apiData.market_count || 0,
    totalVolume: apiData.total_volume || 0,
    participants: apiData.participants || 0,
    risk_tier: apiData.risk_tier || 'Medium',
    category: apiData.category || 'General',
  };
}

// Hook for getting verse details
export function useVerse(verseId: string) {
  const [verse, setVerse] = useState<VerseData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchVerse = async () => {
      if (!verseId) return;
      
      setLoading(true);
      setError(null);

      try {
        const response = await fetch(`/api/verses/${verseId}`);
        if (!response.ok) {
          throw new Error('Failed to fetch verse');
        }

        const data = await response.json();
        setVerse(transformVerseData(data));
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load verse');
      } finally {
        setLoading(false);
      }
    };

    fetchVerse();
  }, [verseId]);

  return { verse, loading, error };
}

// Hook for testing verse matching
export function useVerseMatch(marketTitle: string) {
  const [matchedVerses, setMatchedVerses] = useState<VerseData[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const testMatch = useCallback(async () => {
    if (!marketTitle) return;
    
    setLoading(true);
    setError(null);

    try {
      const response = await fetch('/api/test/verse-match', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ market_title: marketTitle }),
      });

      if (!response.ok) {
        throw new Error('Failed to test verse match');
      }

      const data = await response.json();
      const verses = (data.matched_verses || []).map(transformVerseData);
      setMatchedVerses(verses);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to test match');
    } finally {
      setLoading(false);
    }
  }, [marketTitle]);

  useEffect(() => {
    testMatch();
  }, [testMatch]);

  return { matchedVerses, loading, error, refetch: testMatch };
}
