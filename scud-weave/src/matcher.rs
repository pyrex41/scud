//! Glob pattern matching for event patterns.

use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};

/// A glob pattern for matching file paths and targets.
///
/// Wraps globset for efficient glob matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobPattern {
    pattern: String,
    #[serde(skip)]
    matcher: Option<GlobMatcher>,
}

impl GlobPattern {
    /// Create a new glob pattern from a string.
    pub fn new(pattern: &str) -> Self {
        let matcher = Glob::new(pattern).ok().map(|g| g.compile_matcher());
        GlobPattern {
            pattern: pattern.to_string(),
            matcher,
        }
    }

    /// Check if a string matches this pattern.
    pub fn matches(&self, text: &str) -> bool {
        // "-" means "everything" (SCG convention)
        if self.pattern == "-" {
            return true;
        }
        if let Some(ref m) = self.matcher {
            return m.is_match(text);
        }
        // Fallback: if pattern didn't compile, try exact match
        self.pattern == text
    }

    /// Ensure the compiled matcher is populated (needed after deserialization).
    pub fn ensure_compiled(&mut self) {
        if self.matcher.is_none() {
            self.matcher = Glob::new(&self.pattern).ok().map(|g| g.compile_matcher());
        }
    }

    /// Get the raw pattern string.
    pub fn as_str(&self) -> &str {
        &self.pattern
    }
}

impl PartialEq for GlobPattern {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl std::fmt::Display for GlobPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let pat = GlobPattern::new("src/main.rs");
        assert!(pat.matches("src/main.rs"));
        assert!(!pat.matches("src/lib.rs"));
    }

    #[test]
    fn test_wildcard_all() {
        let pat = GlobPattern::new("**");
        assert!(pat.matches("anything"));
        assert!(pat.matches("src/foo/bar.rs"));
    }

    #[test]
    fn test_directory_glob() {
        let pat = GlobPattern::new("src/**");
        assert!(pat.matches("src/main.rs"));
        assert!(pat.matches("src/foo/bar.rs"));
        assert!(!pat.matches("tests/test.rs"));
    }

    #[test]
    fn test_dash_means_everything() {
        let pat = GlobPattern::new("-");
        assert!(pat.matches("anything"));
    }

    #[test]
    fn test_star_glob() {
        let pat = GlobPattern::new("*.rs");
        assert!(pat.matches("main.rs"));
        // globset matches * across path separators by default
        assert!(pat.matches("src/main.rs"));
    }

    #[test]
    fn test_complex_glob() {
        let pat = GlobPattern::new("src/**/*.rs");
        assert!(pat.matches("src/weave/mod.rs"));
        assert!(pat.matches("src/a/b/c.rs"));
        assert!(!pat.matches("tests/test.rs"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let pat = GlobPattern::new("src/**");
        let json = serde_json::to_string(&pat).unwrap();
        let mut parsed: GlobPattern = serde_json::from_str(&json).unwrap();
        parsed.ensure_compiled();
        assert!(parsed.matches("src/main.rs"));
        assert!(!parsed.matches("tests/test.rs"));
    }
}
