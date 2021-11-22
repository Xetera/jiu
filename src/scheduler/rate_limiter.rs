use governor::{
    clock::QuantaClock,
    state::{DirectStateStore, InMemoryState, NotKeyed},
    RateLimiter,
};

/// Most providers use rate limiter at the domain level and not at the page level
/// in order to prevent exceeding rate limits imposed by webservers
pub type UnscopedLimiter = RateLimiter<NotKeyed, InMemoryState, QuantaClock>;

/// Some providers can use rate limiting at the page level imposed by set limits of API keys
#[allow(dead_code)]
pub type ScopedLimiter = RateLimiter<dyn DirectStateStore, InMemoryState, QuantaClock>;

/// Global rate limiting wrapper for limits imposed on individual providers being run concurrently
pub struct GlobalRateLimiter(UnscopedLimiter);
