import { VerseData } from '../components/verse/VerseCard';

// Verse type constants
export const VerseType = {
  MAIN: 0,
  QUANTUM: 1,
  DISTRIBUTION: 2,
  ROOT: 3,
  CATEGORY: 4,
  SUBCATEGORY: 5,
  MARKET: 6,
} as const;

// Maximum values
export const MAX_LEVERAGE = 100;
export const MAX_VERSE_DEPTH = 4;
export const MAX_SELECTED_VERSES = 3;

// Calculate total multiplier from multiple verses
export function calculateTotalMultiplier(verses: VerseData[]): number {
  const multiplier = verses.reduce((total, verse) => total * verse.multiplier, 1);
  return Math.min(multiplier, MAX_LEVERAGE);
}

// Check if verses are compatible for stacking
export function areVersesCompatible(verse1: VerseData, verse2: VerseData): boolean {
  // Don't allow stacking verses from the same category at high risk
  if (verse1.category === verse2.category && 
      (verse1.risk_tier === 'High' || verse2.risk_tier === 'High')) {
    return false;
  }
  
  // Don't allow total multiplier to exceed max
  const totalMultiplier = verse1.multiplier * verse2.multiplier;
  if (totalMultiplier > MAX_LEVERAGE) {
    return false;
  }
  
  return true;
}

// Get verse color based on category
export function getVerseColor(category: string): string {
  const categoryLower = category.toLowerCase();
  
  if (categoryLower.includes('politics')) return '#7B3FF2';
  if (categoryLower.includes('crypto')) return '#00D4FF';
  if (categoryLower.includes('sports')) return '#4CD964';
  if (categoryLower.includes('science') || categoryLower.includes('tech')) return '#FF9500';
  if (categoryLower.includes('entertainment')) return '#FF3B30';
  if (categoryLower.includes('economics')) return '#FFD60A';
  
  return '#FFD60A'; // Default gold
}

// Get verse icon based on category
export function getVerseIcon(category: string): string {
  const categoryLower = category.toLowerCase();
  
  const iconMap: Record<string, string> = {
    politics: '🏛️',
    crypto: '₿',
    sports: '⚽',
    science: '🔬',
    tech: '💻',
    entertainment: '🎬',
    economics: '📈',
    'us politics': '🇺🇸',
    'world politics': '🌍',
    bitcoin: '₿',
    ethereum: 'Ξ',
    defi: '🏦',
    ai: '🤖',
    nfl: '🏈',
    nba: '🏀',
  };
  
  for (const [key, icon] of Object.entries(iconMap)) {
    if (categoryLower.includes(key)) {
      return icon;
    }
  }
  
  return '📊'; // Default chart icon
}

// Calculate risk score from verses
export function calculateRiskScore(verses: VerseData[]): {
  score: number;
  tier: 'Low' | 'Medium' | 'High' | 'Extreme';
} {
  if (verses.length === 0) {
    return { score: 0, tier: 'Low' };
  }
  
  // Base risk from individual verses
  let riskScore = 0;
  for (const verse of verses) {
    switch (verse.risk_tier) {
      case 'Low': riskScore += 1; break;
      case 'Medium': riskScore += 2; break;
      case 'High': riskScore += 3; break;
    }
  }
  
  // Additional risk from multiplier stacking
  const totalMultiplier = calculateTotalMultiplier(verses);
  if (totalMultiplier > 50) riskScore += 3;
  else if (totalMultiplier > 20) riskScore += 2;
  else if (totalMultiplier > 10) riskScore += 1;
  
  // Determine tier
  let tier: 'Low' | 'Medium' | 'High' | 'Extreme';
  if (riskScore >= 9) tier = 'Extreme';
  else if (riskScore >= 6) tier = 'High';
  else if (riskScore >= 3) tier = 'Medium';
  else tier = 'Low';
  
  return { score: riskScore, tier };
}

// Format verse path for display
export function formatVersePath(path: string[]): string {
  return path.join(' > ');
}

// Search verses by query
export function searchVerses(verses: VerseData[], query: string): VerseData[] {
  const searchTerm = query.toLowerCase();
  
  return verses.filter(verse => 
    verse.name.toLowerCase().includes(searchTerm) ||
    verse.description.toLowerCase().includes(searchTerm) ||
    verse.category.toLowerCase().includes(searchTerm)
  );
}

// Sort verses by different criteria
export function sortVerses(
  verses: VerseData[], 
  sortBy: 'multiplier' | 'volume' | 'risk' | 'markets'
): VerseData[] {
  const sorted = [...verses];
  
  switch (sortBy) {
    case 'multiplier':
      return sorted.sort((a, b) => b.multiplier - a.multiplier);
    case 'volume':
      return sorted.sort((a, b) => b.totalVolume - a.totalVolume);
    case 'risk':
      const riskOrder = { 'Low': 0, 'Medium': 1, 'High': 2 };
      return sorted.sort((a, b) => 
        riskOrder[a.risk_tier] - riskOrder[b.risk_tier]
      );
    case 'markets':
      return sorted.sort((a, b) => b.marketCount - a.marketCount);
    default:
      return sorted;
  }
}