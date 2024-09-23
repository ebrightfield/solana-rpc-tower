//! Tower Service based approach to types that implement `solana_client::rpc_sender::RpcSender`,
//! which can then be used to create `RpcClient` instances using `RpcClient::new_sender`.
//! This gives a greater degree of low-level configurability to a RPC client behavior,
//! including rate limiting, request filtering, retry logic, and more.
pub mod middleware;
pub mod service;
#[cfg(test)]
pub mod test_helpers;

pub use service::*;

pub mod prelude {
    pub use crate::builder::{FnClientBuilder, HttpClientBuilder, ServiceBuilderExt};
    pub use crate::middleware::{MaybeEarlyReturnLayer, TooManyRequestsRetry};
    pub use crate::service::{RpcRequest, Value};
    pub use reqwest::Url;
    pub use solana_client::client_error::ClientError;
    pub use tower::{BoxError, ServiceBuilder as RpcClientBuilder};
}
