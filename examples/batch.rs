use serde_json::json;
use solana_rpc_tower::{prelude::*, service::HttpJsonRpcService};

#[tokio::main]
async fn main() {
    let url = Url::try_from("https://api.mainnet-beta.solana.com").unwrap();
    let mut service = HttpJsonRpcService::new(url.clone(), None, None);

    let request_batch = vec![
        (
            RpcRequest::GetBalance.to_string(),
            json!(["EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"]),
        ),
        (RpcRequest::GetVersion.to_string(), json!([])),
        (RpcRequest::GetLatestBlockhash.to_string(), json!([])),
    ];
    let response = service.send_batch(request_batch).await.unwrap();
    println!("response: {:?}", response);
}
