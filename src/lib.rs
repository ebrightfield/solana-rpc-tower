//! Tower Service based approach to types that implement `solana_client::rpc_sender::RpcSender`,
//! which can then be used to create `RpcClient` instances using `RpcClient::new_sender`.
//! This gives a greater degree of low-level configurability to a RPC client behavior,
//! including rate limiting, request filtering, retry logic, and more.
pub mod middleware;
pub mod service;

pub mod prelude {
    pub use crate::middleware::{MaybeEarlyReturnLayer, TooManyRequestsRetry};
    pub use crate::service::{
        builder::{FnClientBuilder, HttpClientBuilder, ServiceBuilderExt},
        parse_response_body::ParseResponseBodyLayer,
        rpc_sender_impl::{RpcClientSender, SolanaClientRequest, SolanaClientResponse},
        HttpJsonRpcLayer, HttpJsonRpcService,
    };
    pub use crate::service::{RpcRequest, Value};
    pub use reqwest::{Client as ReqwestClient, Response as ReqwestResponse, Url};
    pub use solana_client::client_error::ClientError;
    pub use solana_rpc_client_api::response::{Response as SolanaRpcResponse, RpcResponseContext};
    pub use tower::{BoxError, ServiceBuilder as RpcClientBuilder};
}
