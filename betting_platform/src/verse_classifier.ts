import { SHA3 } from 'sha3';

export class VerseClassifier {
    private keywordMap: Map<string, string>;

    constructor() {
        this.keywordMap = new Map();
        
        // Normalization mappings
        this.keywordMap.set('btc', 'bitcoin');
        this.keywordMap.set('eth', 'ethereum');
        this.keywordMap.set('above', '>');
        this.keywordMap.set('below', '<');
        this.keywordMap.set('usd', '$');
    }

    classifyMarket(title: string): string {
        // Normalize title
        const normalized = this.normalizeTitle(title);

        // Extract keywords
        const keywords = this.extractKeywords(normalized);

        // Sort keywords for deterministic hashing
        const sortedKeywords = [...keywords].sort();

        // Hash to create verse ID
        const hash = new SHA3(256);
        hash.update(sortedKeywords.join('_'));
        const result = hash.digest();

        // Convert first 16 bytes to hex string for u128 representation
        return result.slice(0, 16).toString('hex');
    }

    private normalizeTitle(title: string): string {
        let normalized = title.toLowerCase();

        // Remove punctuation
        normalized = normalized.replace(/[^a-z0-9\s]/g, '');

        // Apply keyword mappings
        this.keywordMap.forEach((to, from) => {
            normalized = normalized.replace(new RegExp(`\\b${from}\\b`, 'g'), to);
        });

        return normalized;
    }

    private extractKeywords(text: string): string[] {
        // Split on whitespace and filter stop words
        const stopWords = ['the', 'will', 'be', 'at', 'in', 'on', 'by'];

        return text.split(/\s+/)
            .filter(word => word.length > 0 && !stopWords.includes(word));
    }

    static calculateLevenshteinDistance(s1: string, s2: string): number {
        const len1 = s1.length;
        const len2 = s2.length;
        const matrix: number[][] = Array(len1 + 1)
            .fill(null)
            .map(() => Array(len2 + 1).fill(0));

        for (let i = 0; i <= len1; i++) {
            matrix[i][0] = i;
        }
        for (let j = 0; j <= len2; j++) {
            matrix[0][j] = j;
        }

        for (let i = 1; i <= len1; i++) {
            for (let j = 1; j <= len2; j++) {
                const cost = s1[i - 1] === s2[j - 1] ? 0 : 1;
                matrix[i][j] = Math.min(
                    matrix[i - 1][j] + 1,    // deletion
                    matrix[i][j - 1] + 1,    // insertion
                    matrix[i - 1][j - 1] + cost // substitution
                );
            }
        }

        return matrix[len1][len2];
    }

    // Check if two market titles should map to the same verse
    isSameVerse(title1: string, title2: string): boolean {
        const normalized1 = this.normalizeTitle(title1);
        const normalized2 = this.normalizeTitle(title2);
        
        const distance = VerseClassifier.calculateLevenshteinDistance(normalized1, normalized2);
        return distance < 5; // Threshold from the specification
    }
}