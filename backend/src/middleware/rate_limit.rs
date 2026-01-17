use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    extract::Request,
    middleware::Next,
};
use governor::{state::NotKeyed, state::InMemoryState, Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<RateLimiter<NotKeyed, InMemoryState, governor::clock::DefaultClock>>,
}

impl RateLimitLayer {
    pub fn new(requests: u32, per_seconds: u64) -> Self {
        let quota = Quota::with_period(Duration::from_secs(per_seconds))
            .unwrap()
            .allow_burst(NonZeroU32::new(requests).unwrap());
            
        RateLimitLayer {
            limiter: Arc::new(RateLimiter::direct(quota)),
        }
    }

    pub async fn rate_limit<B>(&self, req: Request<B>) -> Result<Request<B>, Response> {
        match self.limiter.check() {
            Ok(_) => Ok(req),
            Err(_) => {
                let response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    "Rate limit exceeded. Please try again later.",
                );
                Err(response.into_response())
            }
        }
    }
}

// Rate limiting middleware for specific endpoints
pub async fn rate_limit_middleware(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, Response> {
    // Get the rate limiter from extensions or use a default one
    let rate_limit = req
        .extensions()
        .get::<Arc<RateLimitLayer>>()
        .cloned()
        .unwrap_or_else(|| Arc::new(RateLimitLayer::new(100, 60))); // Default: 100 requests per minute

    let req = rate_limit.rate_limit(req).await?;
    Ok(next.run(req).await)
}
