# Solana RPC Tower
Rust crate to enhance your RPC capabilities.

Use cases:
- Rate Limiting and Concurrency Limits
- URL round-robin, load balancing, load shed
- Proxying RPC servers
- Mock responses
- Setting up a cache layer
- Timeouts
- Retries
- Filtering and request validations
- Request preprocessing, response post-processing

All of these capabilities can be bolted onto your existing Rust codebases that use the `solana-client` crate's `RpcClient`.

## How it Works
Tokio's Tower crate provides a [Service trait](https://tokio.rs/blog/2021-05-14-inventing-the-service-trait) that provides
a general abstraction over Request-Response. 

This crate leverages this to create an implementation of the `RpcSender` trait, which can be used to construct
an `RpcClient`. From there, it's business as usual for client-side Solana interaction with Rust, with all of the
custom behavior bolted on!

This crate contains various services and layers which chain together to replicate the usual Solana `RpcClient` functionality.

On top of that, it's very easy to add additional behavior using layers from the Tower service ecosystem.

The Tower `ServiceBuilder` provides the easiest means of adding some of the most common behaviors.
Here's a simple example showing how to add rate-limiting to an RpcClient. See the `examples/` directory for more.

```
use std::time::Duration;

use reqwest::Url;
use solana_devtools_rpc::builder::ServiceBuilderExt;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() {
    let url = Url::try_from("https://api.mainnet-beta.solana.com").unwrap();
    // Adds a rate limit to the client. Tower's `RateLimitLayer` is a simple "n requests per duration d"
    // For leaky bucket or other algorithms, see crates like `tower_governor`.
    let client = ServiceBuilder::new()
        .rate_limit(2, Duration::from_secs(5))
        .http(url)
        .build_rpc_client();
    let _ = client.get_version().await;
}
```

