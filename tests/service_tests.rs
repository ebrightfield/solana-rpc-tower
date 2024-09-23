use futures::future::BoxFuture;
use serde_json::Value;
use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::rpc_request::RpcRequest;
use solana_rpc_tower::prelude::*;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_response::{Response, RpcResponseContext, RpcVersionInfo};
use solana_sdk::pubkey;
use solana_sdk::transport::TransportError;
use std::time::{Duration, Instant};
use std::{str::FromStr, thread::JoinHandle};
use tower::{BoxError, ServiceBuilder};

use crossbeam_channel::unbounded;
use futures::future;
use jsonrpc_core::{IoHandler, Params};
use jsonrpc_http_server::{AccessControlAllowOrigin, DomainsValidation, ServerBuilder};
use reqwest::Url;
use solana_client::rpc_response::RpcBlockhash;
use tracing_subscriber::fmt::format::FmtSpan;

pub fn spawn_tracing() {
    let _ = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(FmtSpan::FULL)
        .try_init();
}

pub fn io_handler_v1() -> IoHandler {
    let mut io = IoHandler::default();
    // Successful request
    io.add_method("getBalance", |_params: Params| {
        future::ok(
            serde_json::to_value(Response {
                context: RpcResponseContext {
                    slot: 100,
                    api_version: None,
                },
                value: 50,
            })
            .unwrap(),
        )
    });
    io.add_method("getVersion", |_params: Params| {
        future::ok(
            serde_json::to_value(RpcVersionInfo {
                solana_core: "1.18.21".to_string(),
                feature_set: Some(99),
            })
            .unwrap(),
        )
    });
    io.add_method("getLatestBlockhash", |_params: Params| {
        future::ok(
            serde_json::to_value(Response {
                context: RpcResponseContext {
                    slot: 100,
                    api_version: None,
                },
                value: RpcBlockhash {
                    blockhash: "deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh".to_string(),
                    last_valid_block_height: 100,
                },
            })
            .unwrap(),
        )
    });
    io
}

/// Create a pair of client / local-server like so:
/// ```rust
/// async fn demo_server_and_client() {
///     let (url, _handle) = spawn_test_server(io_handler_v1());
///     let rpc_client = RpcClientSender::new_http(url).into_rpc_client(None);
///     // if you want to kill the server, `_handle.abort()``
/// }
/// ```
pub fn spawn_test_server(io: IoHandler) -> (Url, JoinHandle<()>) {
    let host = "127.0.0.1:0";
    let (sender, receiver) = unbounded();
    let rpc_addr = host.parse().unwrap();
    let handle = std::thread::spawn(move || {
        let server = ServerBuilder::new(io)
            .threads(1)
            .cors(DomainsValidation::AllowOnly(vec![
                AccessControlAllowOrigin::Any,
            ]))
            .start_http(&rpc_addr)
            .expect("Unable to start RPC server");
        let rpc_addr = Url::from_str(&format!("http://{}", server.address().clone())).unwrap();
        sender.send(rpc_addr).unwrap();
        server.wait();
    });
    let rpc_addr = receiver.recv().unwrap();
    (rpc_addr, handle)
}

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

/// Just try throwing on various [RpcClientBuilder] layers, to make sure
/// they can compose to create an HTTP RPC client.
#[tokio::test]
async fn service_builder_layers() {
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
        ClientError::from(TransportError::Custom("RPC Method not allowed".to_string())).to_string()
    );
}

fn rpc_response_fn(request: SolanaClientRequest) -> BoxFuture<'static, Result<Value, BoxError>> {
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
async fn using_rpc_response_fn() {
    let rpc_client = RpcClientBuilder::new()
        .filter(|req: (RpcRequest, Value)| match &req.0 {
            RpcRequest::GetBalance => Ok(req),
            RpcRequest::GetVersion => Ok(req),
            RpcRequest::GetLatestBlockhash => Ok(req),
            _ => Err(Box::new(ClientError::from(TransportError::Custom(
                "RPC Method not allowed".to_string(),
            ))) as BoxError),
        })
        .with_fn(rpc_response_fn)
        .build_rpc_client();

    let balance = rpc_client
        .get_balance(&pubkey!("deadbeefXjn8o3yroDHxUtKsZZgoy4GPkPPXfouKNHh"))
        .await
        .unwrap();
    assert_eq!(balance, 123456789);
    let result = rpc_client.get_slot().await.unwrap_err();
    assert_eq!(
        result.to_string(),
        ClientError::from(TransportError::Custom("RPC Method not allowed".to_string())).to_string()
    );
}

#[tokio::test]
async fn using_inlined_rpc_response_fn() {
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

#[tokio::test]
async fn low_level_constructors() {
    let (url, _) = spawn_test_server(io_handler_v1());
    let service = RpcClientBuilder::new()
        .layer(ParseResponseBodyLayer)
        .layer(HttpRequestLayer::new(url.clone()))
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
