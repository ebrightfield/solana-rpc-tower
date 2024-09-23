use crate::middleware::TooManyRequestsRetry;
use crate::stats_updater::{StatsUpdater, TransportStats};
use futures::future::BoxFuture;
use reqwest::Url;
use serde_json::Value;
use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_client::RpcClientConfig;
use solana_client::rpc_request::RpcRequest;
use solana_client::rpc_sender::{RpcSender, RpcTransportStats};
use solana_sdk::commitment_config::CommitmentConfig;
use std::future::Future;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use tower::retry::Retry;
use tower::util::Either;
use tower::{BoxError, Service, ServiceBuilder, ServiceExt};

use super::parse_response_body::{ParseResponseBody, ParseResponseBodyLayer};
use super::{HttpRequestBuilderLayer, HttpRequestBuilderService};

/// The data types sent to `RpcSender::send`, grouped into a tuple.
pub type SolanaClientRequest = (RpcRequest, Value);
/// The response type to `RpcSender::send`.
pub type SolanaClientResponse = Result<Value, BoxError>;
/// The return type of an RpcSenderService
pub type RpcSenderResponseFuture =
    BoxFuture<'static, dyn Future<Output = SolanaClientResponse> + Send>;

// Top level service struct.
pub struct RpcClientSender<T> {
    service: Arc<tokio::sync::RwLock<T>>,
    stats: Arc<RwLock<TransportStats>>,
    url: String,
}

impl RpcClientSender<DefaultHttpService> {
    pub fn new_http(url: Url) -> Self {
        let service = default_http_service(url.clone());
        let stats = Arc::new(RwLock::new(TransportStats::default()));
        Self {
            service: Arc::new(tokio::sync::RwLock::new(service)),
            url: url.to_string(),
            stats,
        }
    }
}

impl<S> RpcClientSender<S>
where
    S: Service<SolanaClientRequest, Response = Value, Error = BoxError> + Send + 'static,
    S::Future: Send + 'static,
{
    pub fn new_with_service(url: String, service: S) -> Self {
        let stats = Arc::new(RwLock::new(TransportStats::default()));
        Self {
            service: Arc::new(tokio::sync::RwLock::new(service)),
            url: url.clone(),
            stats,
        }
    }
}

impl<S> RpcClientSender<S>
where
    S: Service<SolanaClientRequest, Response = Value, Error = BoxError> + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    /// Defaults to "finalized" commitment.
    pub fn into_rpc_client(self, commitment: Option<CommitmentConfig>) -> RpcClient {
        let mut config = RpcClientConfig::default();
        config.commitment_config = commitment.unwrap_or(CommitmentConfig::finalized());
        RpcClient::new_sender(self, Default::default())
    }
}

#[async_trait::async_trait]
impl<T> RpcSender for RpcClientSender<T>
where
    T: Service<SolanaClientRequest, Response = Value, Error = BoxError> + Send + Sync + 'static,
    T::Future: Send + 'static,
{
    async fn send(
        &self,
        request: RpcRequest,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let _stats_updater = StatsUpdater::new(self.stats.clone());
        let mut service = self.service.write().await;
        // We are fine with blocking all other write locks while this awaits,
        // because if one is blocked, they are all blocked.
        if let Err(e) = service.ready().await {
            tracing::error!(err=?e);
        }
        let fut = service.call((request, params));
        drop(service);
        let resp = fut.await.map_err(|e| match e.downcast::<ClientError>() {
            Ok(client_error) => *client_error,
            Err(e) => {
                tracing::error!(err=?e);
                ClientError::new_with_request(ClientErrorKind::Custom(format!("{e}")), request)
            }
        })?;
        tracing::info!(rpc_response=?resp);
        Ok(resp)
    }

    fn get_transport_stats(&self) -> RpcTransportStats {
        self.stats.read().unwrap().deref().into()
    }

    fn url(&self) -> String {
        self.url.clone()
    }
}

/// An HTTP client with 429 retry, and parsing certain error types into [ClientError].
pub type DefaultHttpService =
    ParseResponseBody<HttpRequestBuilderService<Retry<TooManyRequestsRetry, reqwest::Client>>>;
pub type HttpServiceOptionalRetry = ParseResponseBody<
    HttpRequestBuilderService<
        Either<Retry<TooManyRequestsRetry, reqwest::Client>, reqwest::Client>,
    >,
>;

pub fn default_http_service(url: Url) -> DefaultHttpService {
    ServiceBuilder::new()
        .layer(ParseResponseBodyLayer)
        .layer(HttpRequestBuilderLayer::new(url))
        .retry(TooManyRequestsRetry::new(4))
        .service(reqwest_client())
}

/// An HTTP client without 429 retry, but which still parses certain error types into [ClientError].
pub type HttpServiceNoRetry = ParseResponseBody<HttpRequestBuilderService<reqwest::Client>>;

pub fn minimal_http_service(url: Url) -> HttpServiceNoRetry {
    ServiceBuilder::new()
        .layer(ParseResponseBodyLayer)
        .layer(HttpRequestBuilderLayer::new(url))
        .service(reqwest_client())
}

/// An HTTP client [Service] from the [reqwest] crate.
pub fn reqwest_client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}

pub fn localnet_url() -> Url {
    "http://localhost:8899".parse().unwrap()
}

pub fn mainnet_url() -> Url {
    "https://api.mainnet-beta.solana.com".parse().unwrap()
}

pub fn devnet_url() -> Url {
    "https://api.devnet.solana.com".parse().unwrap()
}
