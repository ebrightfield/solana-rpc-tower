use reqwest::Url;
use solana_rpc_tower::prelude::*;

#[tokio::main]
async fn main() {
    // This example adds a concurrency limit to the client, throttling the client
    // to a limit of `n` means the service to block whenever there are already `n` in-flight requests.
    let client = RpcClientBuilder::new()
        .concurrency_limit(2)
        .http(Url::try_from("https://api.mainnet-beta.solana.com").unwrap())
        .build_rpc_client();
    let _ = client.get_version().await;
}
