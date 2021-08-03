use governor::{
    clock::QuantaClock,
    state::{DirectStateStore, InMemoryState, NotKeyed},
    Jitter, Quota, RateLimiter,
};
use nonzero_ext::nonzero;
use std::time::Duration;

/// Most providers use rate limiter at the domain level and not at the page level
/// in order to prevent exceeding rate limits imposed by webservers
pub type UnscopedLimiter = RateLimiter<NotKeyed, InMemoryState, QuantaClock>;

/// Some providers can use rate limiting at the page level imposed by set limits of API keys
#[allow(dead_code)]
pub type ScopedLimiter = RateLimiter<dyn DirectStateStore, InMemoryState, QuantaClock>;

/// Global rate limiting wrapper for limits imposed on individual providers being run concurrently
pub struct GlobalRateLimiter(UnscopedLimiter);

const PROVIDER_PROCESSING_LIMIT: u32 = 8;

/// Creates a new rate limiter used by all providers to prevent getting overwhelmed by requests
/// to different websites
pub fn global_rate_limiter() -> GlobalRateLimiter {
    let rate_limiter = RateLimiter::direct(
        Quota::per_minute(nonzero!(60u32)).allow_burst(nonzero!(PROVIDER_PROCESSING_LIMIT)),
    );
    GlobalRateLimiter(rate_limiter)
}

/// Waits for a global rate limit to be up
pub async fn wait_provider_turn(limiter: &GlobalRateLimiter) {
    limiter
        .0
        // jitter to prevent multiple queued providers from trying to run all at once
        // after the rate limiter is available again
        .until_ready_with_jitter(Jitter::up_to(Duration::from_secs(2u64)))
        .await;
}
