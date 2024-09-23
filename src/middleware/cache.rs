use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use serde_json::Value;
use solana_client::rpc_request::RpcRequest;
use tower::{BoxError, Layer, Service};

use crate::rpc_sender_impl::SolanaClientRequest;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    response: Value,
    at: Instant,
}

#[derive(Debug, Clone)]
pub struct ResponseCacheService<S> {
    inner: S,
    request_type: RpcRequest,
    max_cache_age: Duration,
    cached_values: Arc<RwLock<HashMap<Value, CacheEntry>>>,
}

impl<S> ResponseCacheService<S> {
    pub fn new(inner: S, request_type: RpcRequest, max_cache_age: Duration) -> Self {
        Self {
            inner,
            request_type,
            max_cache_age,
            cached_values: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<S> Service<SolanaClientRequest> for ResponseCacheService<S>
where
    S: Service<SolanaClientRequest, Response = Value, Error = BoxError>,
    S::Future: Send + 'static,
{
    type Response = Value;
    type Error = BoxError;

    type Future = BoxFuture<'static, Result<Value, BoxError>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: SolanaClientRequest) -> Self::Future {
        if req.0 == self.request_type {
            if let Some(entry) = self.cached_values.read().unwrap().get(&req.1) {
                if entry.at.elapsed() < self.max_cache_age {
                    return Box::pin(ready(Ok(entry.response.clone())));
                }
            }
            return Box::pin(CachedResponseFuture {
                inner_fut: Box::pin(self.inner.call(req.clone())),
                request: req,
                cached_values: self.cached_values.clone(),
            });
        }
        Box::pin(self.inner.call(req))
    }
}

pub struct ResponseCacheLayer {
    request_type: RpcRequest,
    max_cache_age: Duration,
}

impl ResponseCacheLayer {
    pub fn new(request_type: RpcRequest, max_cache_age: Duration) -> Self {
        Self {
            request_type,
            max_cache_age,
        }
    }
}

impl<S> Layer<S> for ResponseCacheLayer {
    type Service = ResponseCacheService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ResponseCacheService::new(inner, self.request_type, self.max_cache_age)
    }
}

pub struct CachedResponseFuture<F> {
    // The response body is awaited and parsed as JSON-RPC output after this
    inner_fut: Pin<Box<F>>,
    request: SolanaClientRequest,
    cached_values: Arc<RwLock<HashMap<Value, CacheEntry>>>,
}

impl<F> CachedResponseFuture<F> {
    pub fn new(
        fut: F,
        request: SolanaClientRequest,
        cached_values: Arc<RwLock<HashMap<Value, CacheEntry>>>,
    ) -> Self {
        Self {
            inner_fut: Box::pin(fut),
            request,
            cached_values,
        }
    }
}

impl<F> Future for CachedResponseFuture<F>
where
    F: Future<Output = Result<Value, BoxError>> + Send,
{
    type Output = Result<Value, BoxError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner_fut.poll_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => match r {
                Ok(r) => {
                    let entry = CacheEntry {
                        response: r.clone(),
                        at: Instant::now(),
                    };
                    self.cached_values
                        .write()
                        .unwrap()
                        .insert(self.request.1.clone(), entry);
                    Poll::Ready(Ok(r))
                }
                Err(e) => Poll::Ready(Err(e.into())),
            },
        }
    }
}
