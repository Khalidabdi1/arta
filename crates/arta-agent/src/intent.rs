//! Intent matching for rollback queries.
//!
//! `arta agent rollback --to-intent "working auth"` needs to find a past commit
//! whose recorded intent matches a fuzzy query. This module implements the
//! scoring used for that: an exact (normalized) match is best, a substring
//! match next, and otherwise a trigram-overlap similarity in `0.0..=1.0`.
//!
//! The design intentionally avoids pulling in an embedding model — trigram
//! overlap is cheap, dependency-free, and good enough to distinguish "working
//! auth" from "broke the parser".

/// The strength of a match between a query and a candidate intent.
///
/// Ordered so that any exact match beats any substring match, which beats any
/// fuzzy match; fuzzy matches are then ordered by their similarity score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchStrength {
    /// The query and intent are equal after normalization.
    Exact,
    /// The normalized query occurs as a substring of the normalized intent.
    Substring,
    /// Trigram-overlap similarity in `0.0..=1.0` (only kept above a threshold).
    Fuzzy(f32),
    /// No meaningful overlap.
    None,
}

impl MatchStrength {
    /// A single comparable score, higher is a better match. `None` scores below
    /// every real match so it is never selected over one.
    pub fn score(self) -> f32 {
        match self {
            MatchStrength::Exact => 3.0,
            MatchStrength::Substring => 2.0,
            MatchStrength::Fuzzy(s) => 1.0 + s, // in (1.0, 2.0)
            MatchStrength::None => -1.0,
        }
    }

    /// Whether this represents any match at all.
    pub fn is_match(self) -> bool {
        !matches!(self, MatchStrength::None)
    }
}

/// The minimum trigram similarity we treat as a fuzzy match.
const FUZZY_THRESHOLD: f32 = 0.3;

/// Score how well `intent` matches `query`.
pub fn match_strength(query: &str, intent: &str) -> MatchStrength {
    let q = normalize(query);
    let i = normalize(intent);
    if q.is_empty() {
        return MatchStrength::None;
    }
    if q == i {
        return MatchStrength::Exact;
    }
    if i.contains(&q) {
        return MatchStrength::Substring;
    }
    let sim = trigram_similarity(&q, &i);
    if sim >= FUZZY_THRESHOLD {
        MatchStrength::Fuzzy(sim)
    } else {
        MatchStrength::None
    }
}

/// Lowercase and collapse runs of whitespace to single spaces.
fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The set of character trigrams of `s`, with word boundaries padded so short
/// strings still produce trigrams.
fn trigrams(s: &str) -> std::collections::HashSet<[char; 3]> {
    let padded: Vec<char> = std::iter::once(' ')
        .chain(std::iter::once(' '))
        .chain(s.chars())
        .chain(std::iter::once(' '))
        .collect();
    let mut set = std::collections::HashSet::new();
    for window in padded.windows(3) {
        set.insert([window[0], window[1], window[2]]);
    }
    set
}

/// Jaccard similarity of the two strings' trigram sets, in `0.0..=1.0`.
fn trigram_similarity(a: &str, b: &str) -> f32 {
    let ta = trigrams(a);
    let tb = trigrams(b);
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let intersection = ta.intersection(&tb).count() as f32;
    let union = ta.union(&tb).count() as f32;
    intersection / union
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_ignores_case_and_whitespace() {
        assert_eq!(match_strength("Working  Auth", "working auth"), MatchStrength::Exact);
    }

    #[test]
    fn substring_match_is_detected() {
        assert_eq!(
            match_strength("auth", "fix the working auth flow"),
            MatchStrength::Substring
        );
    }

    #[test]
    fn similar_strings_match_fuzzily() {
        let m = match_strength("working authentication", "fixed working authetication");
        assert!(matches!(m, MatchStrength::Fuzzy(_)), "got {m:?}");
    }

    #[test]
    fn unrelated_strings_do_not_match() {
        assert_eq!(
            match_strength("working auth", "rewrite the packfile reader"),
            MatchStrength::None
        );
    }

    #[test]
    fn ordering_prefers_exact_then_substring_then_fuzzy() {
        assert!(MatchStrength::Exact.score() > MatchStrength::Substring.score());
        assert!(MatchStrength::Substring.score() > MatchStrength::Fuzzy(0.9).score());
        assert!(MatchStrength::Fuzzy(0.9).score() > MatchStrength::Fuzzy(0.4).score());
        assert!(MatchStrength::Fuzzy(0.4).score() > MatchStrength::None.score());
    }

    #[test]
    fn empty_query_never_matches() {
        assert_eq!(match_strength("   ", "anything"), MatchStrength::None);
    }
}
