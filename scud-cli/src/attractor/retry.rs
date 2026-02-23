//! Retry policies with exponential backoff and jitter.

use rand::Rng;
use std::time::Duration;

/// Retry policy configuration.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries (max_attempts = max_retries + 1).
    pub max_retries: u32,
    /// Base delay for exponential backoff.
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Backoff multiplier (default 2.0).
    pub multiplier: f64,
    /// Whether to add jitter to the delay.
    pub jitter: bool,
}

impl RetryPolicy {
    /// No retries.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        }
    }

    /// Standard retry policy: 3 retries, exponential backoff.
    pub fn standard() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Aggressive retry policy: 5 retries, fast backoff.
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(15),
            multiplier: 1.5,
            jitter: true,
        }
    }

    /// Linear retry policy: fixed delay between retries.
    pub fn linear(max_retries: u32, delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay: delay,
            max_delay: delay,
            multiplier: 1.0,
            jitter: false,
        }
    }

    /// Patient retry policy: 10 retries, slow backoff.
    pub fn patient() -> Self {
        Self {
            max_retries: 10,
            base_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(120),
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Create a policy from a node's max_retries value.
    pub fn from_max_retries(max_retries: u32) -> Self {
        if max_retries == 0 {
            Self::none()
        } else {
            Self {
                max_retries,
                ..Self::standard()
            }
        }
    }

    /// Create a named preset.
    pub fn from_preset(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "none" => Some(Self::none()),
            "standard" => Some(Self::standard()),
            "aggressive" => Some(Self::aggressive()),
            "patient" => Some(Self::patient()),
            _ => None,
        }
    }

    /// Check if another retry is allowed.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt <= self.max_retries
    }

    /// Calculate the delay before the next retry.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base = self.base_delay.as_secs_f64();
        let delay = base * self.multiplier.powi(attempt.saturating_sub(1) as i32);
        let capped = delay.min(self.max_delay.as_secs_f64());

        if self.jitter {
            let mut rng = rand::thread_rng();
            let jittered = capped * rng.gen_range(0.5..1.0);
            Duration::from_secs_f64(jittered)
        } else {
            Duration::from_secs_f64(capped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_policy() {
        let policy = RetryPolicy::none();
        assert!(!policy.should_retry(1));
        assert!(policy.should_retry(0));
    }

    #[test]
    fn test_standard_policy() {
        let policy = RetryPolicy::standard();
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    #[test]
    fn test_delay_exponential() {
        let policy = RetryPolicy {
            jitter: false,
            ..RetryPolicy::standard()
        };
        let d1 = policy.delay_for_attempt(1);
        let d2 = policy.delay_for_attempt(2);
        assert!(d2 > d1);
    }

    #[test]
    fn test_delay_capped() {
        let policy = RetryPolicy {
            jitter: false,
            max_delay: Duration::from_secs(10),
            ..RetryPolicy::standard()
        };
        let d = policy.delay_for_attempt(100);
        assert!(d <= Duration::from_secs(10));
    }

    #[test]
    fn test_linear_policy() {
        let policy = RetryPolicy::linear(5, Duration::from_secs(3));
        let d1 = policy.delay_for_attempt(1);
        let d2 = policy.delay_for_attempt(2);
        assert_eq!(d1, d2); // Linear: same delay
        assert_eq!(d1, Duration::from_secs(3));
    }

    #[test]
    fn test_from_preset() {
        assert!(RetryPolicy::from_preset("standard").is_some());
        assert!(RetryPolicy::from_preset("none").is_some());
        assert!(RetryPolicy::from_preset("aggressive").is_some());
        assert!(RetryPolicy::from_preset("patient").is_some());
        assert!(RetryPolicy::from_preset("unknown").is_none());
    }

    #[test]
    fn test_from_max_retries() {
        let p = RetryPolicy::from_max_retries(0);
        assert!(!p.should_retry(1));

        let p = RetryPolicy::from_max_retries(5);
        assert!(p.should_retry(5));
        assert!(!p.should_retry(6));
    }
}
