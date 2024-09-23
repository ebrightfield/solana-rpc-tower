use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread::sleep,
    time::Duration,
};

use solana_client::rpc_response::{Response, RpcResponseContext};
use solana_rpc_tower::{middleware::cache::ResponseCacheLayer, prelude::*};
use solana_sdk::{pubkey::Pubkey, transport::TransportError};
use tower::ServiceBuilder;

fn method_not_allowed() -> BoxError {
    Box::new(ClientError::from(TransportError::Custom(
        "RPC Method not allowed".to_string(),
    )))
}

#[tokio::main]
async fn main() {
    let mock_balance = Arc::new(AtomicU64::new(0));
    let client = ServiceBuilder::new()
        .layer(ResponseCacheLayer::new(
            RpcRequest::GetBalance,
            Duration::from_secs(1),
        ))
        .with_fn(move |req| {
            let value = mock_balance.clone();
            async move {
                match req.0 {
                    RpcRequest::GetBalance => {
                        let b = value.fetch_add(10, Ordering::Relaxed);
                        let resp = serde_json::to_value(Response {
                            context: RpcResponseContext {
                                slot: 100,
                                api_version: None,
                            },
                            value: b,
                        })
                        .unwrap();
                        Ok(resp)
                    }
                    _ => Err(method_not_allowed()),
                }
            }
        })
        .build_rpc_client();
    let pubkey = Pubkey::new_unique();
    let balance0 = client.get_balance(&pubkey).await.unwrap();
    let balance1 = client.get_balance(&pubkey).await.unwrap();
    assert_eq!(balance0, balance1);
    sleep(Duration::from_millis(1001));
    let balance2 = client.get_balance(&pubkey).await.unwrap();
    assert_ne!(balance1, balance2);
    let balance3 = client.get_balance(&pubkey).await.unwrap();
    assert_eq!(balance2, balance3);
}
