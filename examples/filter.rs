use reqwest::Url;
use serde_json::Value;
use solana_client::client_error::ClientError;
use solana_rpc_tower::prelude::*;
use solana_sdk::transport::TransportError;

fn method_not_allowed() -> BoxError {
    Box::new(ClientError::from(TransportError::Custom(
        "RPC Method not allowed".to_string(),
    )))
}

#[tokio::main]
async fn main() {
    // The filter layer provides a means of inspecting a request, and either:
    // 1. Forwarding that request unaltered
    // 2. Forwarding a request with content altered or entirely replaced
    // 3. Returning early with some kind of error (which does *not* get forwarded down the pipeline)
    let client = RpcClientBuilder::new()
        .filter(|req: (RpcRequest, Value)| match &req.0 {
            RpcRequest::GetBalance => Ok(req),
            RpcRequest::GetVersion => Ok(req),
            RpcRequest::GetLatestBlockhash => Ok(req),
            _ => Err(method_not_allowed()),
        })
        .http(Url::try_from("https://api.mainnet-beta.solana.com").unwrap())
        .build_rpc_client();

    let _ = client.get_version().await;
}
