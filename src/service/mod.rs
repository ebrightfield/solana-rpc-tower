pub mod builder;
pub mod http_request_builder;
pub mod parse_response_body;
pub mod rpc_client_trait;
pub mod rpc_sender_impl;
pub mod stats_updater;

pub use serde_json::Value;
pub use solana_client::rpc_request::RpcRequest;

pub use http_request_builder::{HttpRequestBuilderLayer, HttpRequestBuilderService};
pub use parse_response_body::{ParseResponseBody, ParseResponseBodyLayer};

#[cfg(test)]
mod tests {
    use super::rpc_sender_impl::SolanaClientRequest;
    use super::*;
    use crate::prelude::RpcClientBuilder;
    use builder::ServiceBuilderExt;
    use futures::future::BoxFuture;
    use http_request_builder::HttpRequestBuilderLayer;
    use parse_response_body::parse_response_body;
    use rpc_sender_impl::RpcClientSender;
    use serde_json::Value;
    use solana_client::client_error::{ClientError, ClientErrorKind};
    use solana_client::rpc_request::RpcRequest;

    use crate::middleware::{MaybeEarlyReturnLayer, TooManyRequestsRetry};
    use crate::test_helpers::*;
    use solana_client::nonblocking::rpc_client::RpcClient;
    use solana_client::rpc_response::{Response, RpcResponseContext, RpcVersionInfo};
    use solana_sdk::pubkey;
    use solana_sdk::transport::TransportError;
    use std::time::{Duration, Instant};
    use tower::{BoxError, ServiceBuilder};

    #[tokio::test]
    async fn respects_inner_service_readiness() {
        let (url, _) = spawn_test_server(io_handler_v1());

        let rpc_client = ServiceBuilder::new()
            .rate_limit(1, Duration::from_millis(600))
            .filter(|req: (RpcRequest, Value)| match &req.0 {
                RpcRequest::GetBalance => Ok(req),
                RpcRequest::GetVersion => Ok(req),
                RpcRequest::GetLatestBlockhash => Ok(req),
                _ => Err(Box::new(ClientError::from(TransportError::Custom(
                    "RPC Method not allowed".to_string(),
                ))) as BoxError),
            })
            .http(url)
            .build_rpc_client();

        let before_first_request = Instant::now();
        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        let elapsed_after_first = before_first_request.elapsed();
        let after_first_request = Instant::now();
        assert!(
            Duration::from_millis(100) > elapsed_after_first,
            "{:?}",
            elapsed_after_first
        );

        let _ = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        let elapsed_after_second = after_first_request.elapsed();
        assert_eq!(balance, 50);
        assert!(
            Duration::from_millis(600) < elapsed_after_first + elapsed_after_second,
            "{:?} + {:?}",
            elapsed_after_first,
            elapsed_after_second
        );
    }

    #[tokio::test]
    async fn service() {
        let (url, _) = spawn_test_server(io_handler_v1());

        let rpc_client = RpcClientBuilder::new()
            .rate_limit(5, Duration::from_secs(60))
            .and_then(|resp| {
                Box::pin(async move {
                    tracing::error!(message = "from inside the `and_then` function", ?resp);
                    Ok(resp)
                })
            })
            .filter(|request| {
                tracing::info!("from inside the `filter` function");
                Result::<_, BoxError>::Ok(request)
            })
            .concurrency_limit(1024)
            .map_future(|future| async move { future.await })
            .layer(MaybeEarlyReturnLayer::new(
                &|req: &RpcRequest, v: &Value| {
                    if let RpcRequest::GetBalance = req {
                        tracing::info!(value=?v);
                        let resp = serde_json::to_value(Response {
                            context: RpcResponseContext {
                                slot: 100,
                                api_version: None,
                            },
                            value: 123456789,
                        })
                        .unwrap();
                        tracing::info!(?resp);
                        return Some(Result::<_, BoxError>::Ok(resp));
                    }
                    None
                },
            ))
            .filter(|req: (RpcRequest, Value)| match &req.0 {
                RpcRequest::GetBalance => Ok(req),
                RpcRequest::GetVersion => Ok(req),
                RpcRequest::GetLatestBlockhash => Ok(req),
                _ => Err(Box::new(ClientError::from(TransportError::Custom(
                    "RPC Method not allowed".to_string(),
                ))) as BoxError),
            })
            .http(url)
            .build_rpc_client();

        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        assert_eq!(balance, 123456789);
        let result = rpc_client.get_slot().await.unwrap_err();
        assert_eq!(
            result.to_string(),
            ClientError::from(TransportError::Custom("RPC Method not allowed".to_string()))
                .to_string()
        );
    }

    fn fake_service(request: SolanaClientRequest) -> BoxFuture<'static, Result<Value, BoxError>> {
        Box::pin(async move {
            let (method, params) = request;
            match method {
                RpcRequest::GetBalance => {
                    tracing::info!(?params);
                    let resp = serde_json::to_value(Response {
                        context: RpcResponseContext {
                            slot: 100,
                            api_version: None,
                        },
                        value: 123456789,
                    })
                    .unwrap();
                    tracing::info!(?resp);
                    Ok(resp)
                }
                RpcRequest::GetVersion => Ok(serde_json::to_value(RpcVersionInfo {
                    solana_core: "1.18.21".to_string(),
                    feature_set: Some(99),
                })
                .unwrap()),
                _ => Err(Box::new(ClientError::new_with_request(
                    ClientErrorKind::Custom("foo".to_string()),
                    method,
                )) as BoxError),
            }
        })
    }

    #[tokio::test]
    async fn service_fn_test() {
        let rpc_client = RpcClientBuilder::new()
            .filter(|req: (RpcRequest, Value)| match &req.0 {
                RpcRequest::GetBalance => Ok(req),
                RpcRequest::GetVersion => Ok(req),
                RpcRequest::GetLatestBlockhash => Ok(req),
                _ => Err(Box::new(ClientError::from(TransportError::Custom(
                    "RPC Method not allowed".to_string(),
                ))) as BoxError),
            })
            .with_fn(fake_service)
            .build_rpc_client();

        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        assert_eq!(balance, 123456789);
        let result = rpc_client.get_slot().await.unwrap_err();
        assert_eq!(
            result.to_string(),
            ClientError::from(TransportError::Custom("RPC Method not allowed".to_string()))
                .to_string()
        );
    }

    #[tokio::test]
    async fn service_fn_test2() {
        let (url, _) = spawn_test_server(io_handler_v1());
        let service = RpcClientBuilder::new()
            .and_then(parse_response_body)
            .layer(HttpRequestBuilderLayer::new(url.clone()))
            .retry(TooManyRequestsRetry::new(4))
            .service(reqwest::Client::builder().build().unwrap());

        let sender = RpcClientSender::new_with_service(url.to_string(), service);

        let rpc_client = RpcClient::new_sender(sender, Default::default());
        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        assert_eq!(balance, 50);
        let result = rpc_client.get_slot().await.unwrap_err();
        assert_eq!(
            result.to_string(),
            ClientError::from(TransportError::Custom(
                "RPC response error -32601: Method not found; ".to_string()
            ))
            .to_string()
        );
    }

    #[tokio::test]
    async fn service_builder_test_3() {
        let rpc_client = RpcClientBuilder::new()
            .filter(|req: (RpcRequest, Value)| match &req.0 {
                RpcRequest::GetBalance => Ok(req),
                RpcRequest::GetVersion => Ok(req),
                RpcRequest::GetLatestBlockhash => Ok(req),
                _ => Err(Box::new(ClientError::from(TransportError::Custom(
                    "RPC Method not allowed".to_string(),
                ))) as BoxError),
            })
            .with_fn(fake_service)
            .build_rpc_client();

        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        assert_eq!(balance, 123456789);
        let result = rpc_client.get_slot().await.unwrap_err();
        assert_eq!(
            result.to_string(),
            ClientError::from(TransportError::Custom("RPC Method not allowed".to_string()))
                .to_string()
        );
        let rpc_client = RpcClientBuilder::new()
            .with_fn(|(method, params)| async move {
                match method {
                    RpcRequest::GetBalance => {
                        tracing::info!(?params);
                        let resp = serde_json::to_value(Response {
                            context: RpcResponseContext {
                                slot: 100,
                                api_version: None,
                            },
                            value: 123456777,
                        })
                        .unwrap();
                        tracing::info!(?resp);
                        Ok(resp)
                    }
                    RpcRequest::GetVersion => Ok(serde_json::to_value(RpcVersionInfo {
                        solana_core: "1.18.21".to_string(),
                        feature_set: Some(99),
                    })
                    .unwrap()),
                    _ => Err(Box::new(ClientError::new_with_request(
                        ClientErrorKind::Custom("foo".to_string()),
                        method,
                    )) as BoxError),
                }
            })
            .build_rpc_client();

        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        assert_eq!(balance, 123456777);
        let result = rpc_client.get_slot().await.unwrap_err();
        assert_eq!(
            result.to_string(),
            ClientError::from(TransportError::Custom("foo".to_string())).to_string()
        );

        let rpc_client = RpcClientBuilder::new()
            .concurrency_limit(4)
            .with_fn(|(method, params)| async move {
                match method {
                    RpcRequest::GetBalance => {
                        tracing::info!(?params);
                        let resp = serde_json::to_value(Response {
                            context: RpcResponseContext {
                                slot: 100,
                                api_version: None,
                            },
                            value: 123456777,
                        })
                        .unwrap();
                        tracing::info!(?resp);
                        Ok(resp)
                    }
                    RpcRequest::GetVersion => Ok(serde_json::to_value(RpcVersionInfo {
                        solana_core: "1.18.21".to_string(),
                        feature_set: Some(99),
                    })
                    .unwrap()),
                    _ => Err(Box::new(ClientError::new_with_request(
                        ClientErrorKind::Custom("foo".to_string()),
                        method,
                    )) as BoxError),
                }
            })
            .build_rpc_client();

        let balance = rpc_client
            .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
            .await
            .unwrap();
        assert_eq!(balance, 123456777);
        let result = rpc_client.get_slot().await.unwrap_err();
        assert_eq!(
            result.to_string(),
            ClientError::from(TransportError::Custom("foo".to_string())).to_string()
        );
    }
}
