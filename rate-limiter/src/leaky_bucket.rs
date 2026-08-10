use std::{time::{Duration, Instant}};

use crate::Limiter;

pub struct LeakyBucketRateLimiter {
    pub max_bucket_size: usize,
    pub outflow_rate: usize, // tasks per second
    tokens: usize,
    last_reset: Instant,
}

impl Limiter for LeakyBucketRateLimiter {
    fn try_acquire(&mut self) -> bool {
        let whole_secs = self.last_reset.elapsed().as_secs();

        if whole_secs > 0 && self.tokens > 0 {
            // Drain only whole seconds, so the sub-second remainder carries into
            // the next call instead of being discarded.
            self.tokens = self
                .tokens
                .saturating_sub(self.outflow_rate.saturating_mul(whole_secs as usize));
            self.last_reset += Duration::from_secs(whole_secs);
        }
        if self.tokens < self.max_bucket_size {
            self.tokens += 1;
            true
        } else {
            false
        }
    }
}

impl LeakyBucketRateLimiter {
    pub fn new(max_bucket_size: usize, outflow_rate: usize) -> LeakyBucketRateLimiter {
        LeakyBucketRateLimiter {
            max_bucket_size,
            outflow_rate,
            tokens: 0,
            last_reset: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn starts_empty() {
        let mut limiter = LeakyBucketRateLimiter::new(3, 3);
        assert!((0..3).all(|_| limiter.try_acquire()));
    }

    #[test]
    fn rejects_once_full() {
        let mut limiter = LeakyBucketRateLimiter::new(2, 3);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire()); // boom
    }

    /// A slow drain must not shrink the burst the bucket can hold.
    #[test]
    fn burst_is_bounded_by_size_not_rate() {
        let mut limiter = LeakyBucketRateLimiter::new(2, 1);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn empties_after_per_second() {
        let mut limiter = LeakyBucketRateLimiter::new(1, 1);
        assert!(limiter.try_acquire());
        sleep(Duration::from_millis(1100));
        assert!(limiter.try_acquire());
    }

    #[test]
    fn partial_interval_earns_nothing() {
        let mut limiter = LeakyBucketRateLimiter::new(1, 1);
        assert!(limiter.try_acquire());
        sleep(Duration::from_millis(50));
        assert!(!limiter.try_acquire());
    }
}
