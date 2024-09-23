#![cfg(test)]

use std::{str::FromStr, thread::JoinHandle};

use crossbeam_channel::unbounded;
use futures::future;
use jsonrpc_core::{IoHandler, Params};
use jsonrpc_http_server::{AccessControlAllowOrigin, DomainsValidation, ServerBuilder};
use reqwest::Url;
use solana_client::rpc_response::{Response, RpcBlockhash, RpcResponseContext, RpcVersionInfo};
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
