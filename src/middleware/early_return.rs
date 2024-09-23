use crate::service::rpc_sender_impl::{SolanaClientRequest, SolanaClientResponse};
use futures::future::BoxFuture;
use serde_json::Value;
use solana_client::rpc_request::RpcRequest;
use std::future::ready;
use std::task::{Context, Poll};
use tower::{BoxError, Layer, Service};

#[derive(Debug)]
pub struct MaybeEarlyReturn<S, F> {
    inner: S,
    f: F,
}

impl<S, F> MaybeEarlyReturn<S, F> {
    pub fn new(s: S, f: F) -> Self {
        Self { inner: s, f }
    }
}

impl<S, F> Service<SolanaClientRequest> for MaybeEarlyReturn<S, F>
where
    S: Service<SolanaClientRequest, Response = Value, Error = BoxError>,
    S::Future: Send + 'static,
    F: for<'a> Fn(&'a RpcRequest, &'a Value) -> Option<SolanaClientResponse>,
{
    type Response = Value;
    type Error = BoxError;

    type Future = BoxFuture<'static, Result<Value, BoxError>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: SolanaClientRequest) -> Self::Future {
        match (self.f)(&req.0, &req.1) {
            None => Box::pin(self.inner.call(req)),
            Some(result) => Box::pin(ready(result)),
        }
    }
}

pub struct MaybeEarlyReturnLayer<F>(F);

impl<F> MaybeEarlyReturnLayer<F> {
    pub fn new(f: F) -> Self {
        Self(f)
    }
}

impl<S, F> Layer<S> for MaybeEarlyReturnLayer<F>
where
    F: for<'a> Fn(&'a RpcRequest, &'a Value) -> Option<SolanaClientResponse> + Clone,
{
    type Service = MaybeEarlyReturn<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        MaybeEarlyReturn::new(inner, self.0.clone())
    }
}
