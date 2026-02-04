use solana_program::program_error::ProgramError;

/// Calculate Levenshtein distance between two strings
pub fn calculate_levenshtein_distance(s1: &str, s2: &str) -> Result<usize, ProgramError> {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    
    // Create matrix with dimensions (len1 + 1) x (len2 + 1)
    let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];
    
    // Initialize first row and column
    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }
    
    // Fill the matrix
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();
    
    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
            
            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1,      // deletion
                    matrix[i][j - 1] + 1       // insertion
                ),
                matrix[i - 1][j - 1] + cost    // substitution
            );
        }
    }
    
    Ok(matrix[len1][len2])
}

/// Check if two strings are similar based on Levenshtein distance
pub fn are_similar(s1: &str, s2: &str, threshold: u8) -> Result<bool, ProgramError> {
    let distance = calculate_levenshtein_distance(s1, s2)?;
    Ok(distance < threshold as usize)
}

/// Find the most similar string from a list
pub fn find_most_similar<'a>(
    target: &str,
    candidates: &'a [String],
) -> Result<Option<(&'a String, usize)>, ProgramError> {
    let mut best_match = None;
    let mut min_distance = usize::MAX;
    
    for candidate in candidates {
        let distance = calculate_levenshtein_distance(target, candidate)?;
        if distance < min_distance {
            min_distance = distance;
            best_match = Some((candidate, distance));
        }
    }
    
    Ok(best_match)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_identical_strings() {
        assert_eq!(calculate_levenshtein_distance("test", "test").unwrap(), 0);
        assert_eq!(calculate_levenshtein_distance("", "").unwrap(), 0);
    }
    
    #[test]
    fn test_single_operations() {
        // Single insertion
        assert_eq!(calculate_levenshtein_distance("test", "tests").unwrap(), 1);
        
        // Single deletion
        assert_eq!(calculate_levenshtein_distance("tests", "test").unwrap(), 1);
        
        // Single substitution
        assert_eq!(calculate_levenshtein_distance("test", "best").unwrap(), 1);
    }
    
    #[test]
    fn test_multiple_operations() {
        assert_eq!(calculate_levenshtein_distance("kitten", "sitting").unwrap(), 3);
        assert_eq!(calculate_levenshtein_distance("saturday", "sunday").unwrap(), 3);
    }
    
    #[test]
    fn test_similarity_check() {
        assert!(are_similar("bitcoin", "bitcion", 5).unwrap()); // distance = 2
        assert!(are_similar("btc 150k", "btc 155k", 5).unwrap()); // distance = 1
        assert!(!are_similar("bitcoin", "ethereum", 5).unwrap()); // distance > 5
    }
    
    #[test]
    fn test_find_most_similar() {
        let candidates = vec![
            "bitcoin".to_string(),
            "bitcion".to_string(),
            "ethereum".to_string(),
            "btc".to_string(),
        ];
        
        let result = find_most_similar("bitcon", &candidates).unwrap();
        assert!(result.is_some());
        
        // "bitcon" -> "bitcoin" = 1 insertion
        // "bitcon" -> "bitcion" = 1 substitution
        // Both have distance 1, but "bitcoin" comes first in the list
        let (best_match, distance) = result.unwrap();
        assert_eq!(best_match, "bitcoin");
        assert_eq!(distance, 1);
    }
}