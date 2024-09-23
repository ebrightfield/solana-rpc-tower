use std::future::Future;

use reqwest::Url;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use tower::{
    retry::RetryLayer, service_fn, util::ServiceFn, BoxError, Layer, Service, ServiceBuilder,
};

use crate::middleware::TooManyRequestsRetry;

use super::{
    rpc_sender_impl::{
        reqwest_client, HttpServiceOptionalRetry, RpcClientSender, SolanaClientRequest,
        SolanaClientResponse,
    },
    HttpRequestBuilderLayer, ParseResponseBodyLayer,
};

pub trait ServiceBuilderExt<L> {
    fn http(self, url: Url) -> HttpClientBuilder<L>;
    fn with_fn<S, F>(self, f: S) -> FnClientBuilder<L, S>
    where
        S: FnMut(SolanaClientRequest) -> F + Send + 'static,
        F: Future<Output = SolanaClientResponse> + Send + 'static;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    fn http(self, url: Url) -> HttpClientBuilder<L> {
        HttpClientBuilder {
            service_builder: self,
            retry_429: 5,
            url,
            commitment: None,
        }
    }

    fn with_fn<S, F>(self, f: S) -> FnClientBuilder<L, S>
    where
        S: FnMut(SolanaClientRequest) -> F + Send + 'static,
        F: Future<Output = SolanaClientResponse> + Send + 'static,
    {
        FnClientBuilder {
            service_builder: self,
            f,
            commitment: None,
            mock_url: None,
        }
    }
}

pub struct HttpClientBuilder<L> {
    service_builder: ServiceBuilder<L>,
    retry_429: usize,
    url: Url,
    commitment: Option<CommitmentConfig>,
}

impl<L, S> HttpClientBuilder<L>
where
    L: Layer<HttpServiceOptionalRetry, Service = S>,
    S: Service<SolanaClientRequest, Response = Value, Error = BoxError> + Send + Sync + 'static,
    S::Future: Send + 'static,
{
    pub fn retry_429(mut self, n_times: usize) -> Self {
        self.retry_429 = n_times;
        self
    }

    pub fn commitment(mut self, commitment: CommitmentConfig) -> Self {
        self.commitment = Some(commitment);
        self
    }

    pub fn build_rpc_client(self) -> RpcClient {
        let Self {
            service_builder,
            retry_429,
            url,
            commitment,
        } = self;
        let retry_layer =
            (retry_429 > 0).then(|| RetryLayer::new(TooManyRequestsRetry::new(retry_429)));
        let url_str = url.to_string();
        let service = service_builder
            .layer(ParseResponseBodyLayer)
            .layer(HttpRequestBuilderLayer::new(url))
            .option_layer(retry_layer)
            .service(reqwest_client());
        RpcClientSender::new_with_service(url_str, service).into_rpc_client(commitment)
    }
}

pub struct FnClientBuilder<L, F> {
    service_builder: ServiceBuilder<L>,
    f: F,
    commitment: Option<CommitmentConfig>,
    mock_url: Option<String>,
}

impl<L, F> FnClientBuilder<L, F> {
    pub fn commitment(mut self, commitment: CommitmentConfig) -> Self {
        self.commitment = Some(commitment);
        self
    }

    pub fn mock_url(mut self, mock_url: String) -> Self {
        self.mock_url = Some(mock_url);
        self
    }
}

impl<L, F, S, T> FnClientBuilder<L, S>
where
    L: Layer<ServiceFn<S>, Service = T>,
    S: FnMut(SolanaClientRequest) -> F + Send + 'static,
    F: Future<Output = SolanaClientResponse> + Send + 'static,
    T: Service<SolanaClientRequest, Response = Value, Error = BoxError> + Send + Sync + 'static,
    T::Future: Send + 'static,
{
    pub fn build_rpc_client(self) -> RpcClient {
        let Self {
            service_builder,
            f,
            commitment,
            mock_url,
        } = self;
        let service = service_builder.service(service_fn(f));
        RpcClientSender::new_with_service(mock_url.unwrap_or_default(), service)
            .into_rpc_client(commitment)
    }
}
