pub mod leaky_bucket;
pub mod token_bucket;

use std::sync::Mutex;

pub trait Limiter {
    fn try_acquire(&mut self) -> bool;
}

/// Shares one limiter across threads. Wrap it in an `Arc` to hand out handles.
///
/// An `Arc` only ever lends out `&self`, and`Limiter::try_acquire` wants
/// `&mut self`, so the lock is what bridges them.
pub struct RateLimiterLayer<L> {
    pub rate_limiter: Mutex<L>,
}

impl<L: Limiter> RateLimiterLayer<L> {
    pub fn new(rate_limiter: L) -> Self {
        Self {
            rate_limiter: Mutex::new(rate_limiter),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.rate_limiter.lock().unwrap().try_acquire()
    }
}
