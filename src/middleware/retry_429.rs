use reqwest::header::RETRY_AFTER;
use reqwest::StatusCode;
use std::time::Duration;
use tokio::time::Sleep;
use tower::retry;

#[derive(Debug, Clone)]
pub struct TooManyRequestsRetry {
    retries_remaining: usize,
    rate_limited_time: Duration,
}

impl TooManyRequestsRetry {
    pub fn new(num_retries: usize) -> Self {
        Self {
            retries_remaining: num_retries,
            rate_limited_time: Default::default(),
        }
    }
}

impl retry::Policy<reqwest::Request, reqwest::Response, reqwest::Error> for TooManyRequestsRetry {
    type Future = Sleep;

    fn retry(
        &mut self,
        _req: &mut reqwest::Request,
        result: &mut Result<reqwest::Response, reqwest::Error>,
    ) -> Option<Self::Future> {
        if let Ok(response) = result {
            if !response.status().is_success() {
                if response.status() == StatusCode::TOO_MANY_REQUESTS && self.retries_remaining > 0
                {
                    let mut duration = Duration::from_millis(500);
                    if let Some(retry_after) = response.headers().get(RETRY_AFTER) {
                        if let Ok(retry_after) = retry_after.to_str() {
                            if let Ok(retry_after) = retry_after.parse::<u64>() {
                                if retry_after < 120 {
                                    duration = Duration::from_secs(retry_after);
                                }
                            }
                        }
                    }

                    self.retries_remaining -= 1;
                    tracing::debug!(
                                "Too many requests: server responded with {:?}, {} retries left, pausing for {:?}",
                                response, self.retries_remaining, duration
                            );

                    self.rate_limited_time += duration;
                    return Some(tokio::time::sleep(duration));
                }
            }
        }
        None
    }

    fn clone_request(&mut self, req: &reqwest::Request) -> Option<reqwest::Request> {
        let mut request = reqwest::Request::new(req.method().clone(), req.url().clone());
        *request.headers_mut() = req.headers().clone();
        *request.timeout_mut() = req.timeout().copied().clone();
        *request.body_mut() = req
            .body()
            .and_then(|b| b.as_bytes())
            .map(|bytes| bytes.to_vec())
            .map(Into::into);
        Some(request)
    }
}
