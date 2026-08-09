use std::{
    cmp,
    time::{Duration, Instant},
};

use crate::Limiter;

pub struct TokenBucketRateLimiter {
    pub bucket_size: usize,
    pub tokens_per_second: usize,

    tokens: usize,
    last_refill: Instant,
}

impl Limiter for TokenBucketRateLimiter {
    fn try_acquire(&mut self) -> bool {
        if self.tokens < self.bucket_size {
            let whole_secs = self.last_refill.elapsed().as_secs();
            let tokens_to_add = (whole_secs as usize).saturating_mul(self.tokens_per_second);
            if tokens_to_add > 0 {
                self.tokens = cmp::min(self.tokens + tokens_to_add, self.bucket_size);
                // Credit only whole seconds, so the sub-second remainder carries into
                // the next call instead of being discarded.
                self.last_refill += Duration::from_secs(whole_secs);
            }
        }

        if self.tokens > 0 {
            self.tokens -= 1;
            return true;
        }
        false
    }
}

impl TokenBucketRateLimiter {
    pub fn new(bucket_size: usize, tokens_per_second: usize) -> TokenBucketRateLimiter {
        assert!(tokens_per_second > 0, "refill rate must be non-zero");
        TokenBucketRateLimiter {
            bucket_size,
            tokens_per_second,
            tokens: bucket_size,
            last_refill: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn starts_full() {
        let mut limiter = TokenBucketRateLimiter::new(3, 1);
        assert!((0..3).all(|_| limiter.try_acquire()));
    }

    #[test]
    fn rejects_once_drained() {
        let mut limiter = TokenBucketRateLimiter::new(2, 1);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire()); // boom
    }

    #[test]
    fn refills_after_one_interval() {
        let mut limiter = TokenBucketRateLimiter::new(1, 1);
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
        sleep(Duration::from_millis(1100));
        assert!(limiter.try_acquire());
    }

    #[test]
    fn refill_caps_at_bucket_size() {
        let mut limiter = TokenBucketRateLimiter::new(2, 4);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        sleep(Duration::from_millis(1100)); // enough for 4 tokens, bucket holds 2
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn partial_interval_earns_nothing() {
        let mut limiter = TokenBucketRateLimiter::new(1, 1);
        assert!(limiter.try_acquire());
        sleep(Duration::from_millis(50));
        assert!(!limiter.try_acquire());
    }

    #[test]
    #[should_panic]
    fn zero_refill_rate_panics() {
        TokenBucketRateLimiter::new(1, 0);
    }
}
