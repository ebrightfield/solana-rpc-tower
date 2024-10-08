use std::time::Duration;

use solana_rpc_tower::prelude::*;

#[tokio::main]
async fn main() {
    let url = Url::try_from("https://api.mainnet-beta.solana.com").unwrap();
    // Adds a rate limit to the client. Tower's `RateLimitLayer` is a simple "n requests per duration d"
    // For leaky bucket or other algorithms, see crates like `tower_governor`.
    let client = RpcClientBuilder::new()
        .rate_limit(2, Duration::from_secs(5))
        .http(url)
        .build_rpc_client();
    let _ = client.get_version().await;
}
