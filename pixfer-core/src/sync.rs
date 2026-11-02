//! Sync-word detection for the camera-facing color-block stream.
//! Hilariously over-engineered: three bits get a builder, scorer, and report card.

/// A validated sequence of 3-bit color-block symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPattern {
    symbols: Vec<u8>,
}

impl SyncPattern {
    /// Creates an empty pattern which can be populated with [`Self::push`].
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }

    /// Creates a pattern from symbols, retaining only their three low bits.
    pub fn from_symbols(symbols: &[u8]) -> Self {
        let mut pattern = Self::new();
        for &symbol in symbols {
            pattern.push(symbol);
        }
        pattern
    }

    /// Appends one color-block symbol, normalized to the 3-bit wire alphabet.
    pub fn push(&mut self, symbol: u8) -> &mut Self {
        self.symbols.push(symbol & 0b111);
        self
    }

    /// Appends several normalized symbols and returns the builder.
    pub fn extend(&mut self, symbols: &[u8]) -> &mut Self {
        for &symbol in symbols {
            self.push(symbol);
        }
        self
    }

    /// Returns the number of symbols in the word.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Returns whether the word contains no symbols.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Returns the normalized symbols as a slice.
    pub fn as_slice(&self) -> &[u8] {
        &self.symbols
    }

    /// Consumes the builder and returns its symbols.
    pub fn into_symbols(self) -> Vec<u8> {
        self.symbols
    }
}

impl Default for SyncPattern {
    fn default() -> Self {
        Self::new()
    }
}

/// A sync detector with a bounded mismatch budget.
#[derive(Debug, Clone)]
pub struct SyncDetector {
    pattern: SyncPattern,
    tolerance: usize,
}

impl SyncDetector {
    /// Creates a detector for `pattern` with zero tolerated mismatches.
    pub fn new(pattern: SyncPattern) -> Self {
        Self {
            pattern,
            tolerance: 0,
        }
    }

    /// Sets the maximum Hamming-like mismatch count accepted by [`Self::find`].
    pub fn with_tolerance(mut self, tolerance: usize) -> Self {
        self.tolerance = tolerance.min(self.pattern.len());
        self
    }

    /// Returns the configured pattern.
    pub fn pattern(&self) -> &SyncPattern {
        &self.pattern
    }

    /// Returns the configured mismatch budget.
    pub fn tolerance(&self) -> usize {
        self.tolerance
    }

    /// Computes mismatches at `start`, or `None` when a complete window is absent.
    pub fn score_at(&self, stream: &[u8], start: usize) -> Option<usize> {
        let end = start.checked_add(self.pattern.len())?;
        if self.pattern.is_empty() || end > stream.len() {
            return None;
        }
        Some(
            stream[start..end]
                .iter()
                .zip(self.pattern.as_slice())
                .filter(|(actual, expected)| (*actual & 7) != **expected)
                .count(),
        )
    }

    /// Finds the first window within tolerance and returns its stream index.
    pub fn find(&self, stream: &[u8]) -> Option<usize> {
        self.best_match(stream)
            .and_then(|(index, score)| (score <= self.tolerance).then_some(index))
    }

    /// Finds the lowest-scoring complete window, preferring the earliest tie.
    pub fn best_match(&self, stream: &[u8]) -> Option<(usize, usize)> {
        if self.pattern.is_empty() || stream.len() < self.pattern.len() {
            return None;
        }
        let limit = stream
            .len()
            .saturating_sub(self.pattern.len())
            .saturating_add(1);
        let mut best = None;
        for index in 0..limit {
            if let Some(score) = self.score_at(stream, index) {
                if best.map_or(true, |(_, old)| score < old) {
                    best = Some((index, score));
                }
            }
        }
        best
    }

    /// Returns every candidate whose score is no greater than tolerance.
    pub fn candidates(&self, stream: &[u8]) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        if self.pattern.is_empty() || stream.len() < self.pattern.len() {
            return result;
        }
        let limit = stream
            .len()
            .saturating_sub(self.pattern.len())
            .saturating_add(1);
        for index in 0..limit {
            if let Some(score) = self.score_at(stream, index) {
                if score <= self.tolerance {
                    result.push((index, score));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn detector(tolerance: usize) -> SyncDetector {
        SyncDetector::new(SyncPattern::from_symbols(&[7, 0, 7, 3, 1])).with_tolerance(tolerance)
    }
    #[test]
    fn exact_match() {
        assert_eq!(detector(0).find(&[2, 7, 0, 7, 3, 1, 4]), Some(1));
    }
    #[test]
    fn off_by_one_offset() {
        assert_eq!(detector(0).find(&[7, 0, 7, 3, 1]), Some(0));
    }
    #[test]
    fn tolerance_match() {
        assert_eq!(detector(1).find(&[7, 0, 6, 3, 1]), Some(0));
    }
    #[test]
    fn no_match() {
        assert_eq!(detector(0).find(&[1, 2, 3, 4, 5]), None);
    }
    #[test]
    fn empty_stream() {
        assert_eq!(detector(1).find(&[]), None);
    }
    #[test]
    fn normalization_and_builder() {
        let mut p = SyncPattern::new();
        p.push(8).extend(&[9, 2]);
        assert_eq!(p.as_slice(), &[0, 1, 2]);
        assert_eq!(p.len(), 3);
    }
    #[test]
    fn best_match_reports_score() {
        assert_eq!(detector(0).best_match(&[0, 7, 0, 6, 3, 1]), Some((1, 1)));
    }
    #[test]
    fn candidates_include_scores() {
        assert_eq!(
            detector(1).candidates(&[7, 0, 7, 3, 1, 7, 0, 6, 3, 1]),
            vec![(0, 0), (5, 1)]
        );
    }
}
