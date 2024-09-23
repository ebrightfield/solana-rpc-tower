use solana_rpc_tower::prelude::*;

#[tokio::main]
async fn main() {
    let url = Url::try_from("https://api.mainnet-beta.solana.com").unwrap();
    // This constructs a client that behaves the same as the vanilla Solana RPC client.
    let client = RpcClientSender::new_http(url.clone()).into_rpc_client(None);
    let _ = client.get_version().await;

    // This constructs a much more minimal client without 429 retries.
    let client = RpcClientBuilder::new()
        .http(url)
        .retry_429(0)
        .build_rpc_client();
    let _ = client.get_version().await;
}
