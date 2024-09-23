# Solana RPC Tower
Enhance your Rust Solana clients with services and layers from the Tower ecosystem.

Use cases:
- Rate Limiting and Concurrency Limits
- URL round-robin, load balancing, load shed
- Proxying RPC servers
- Mock responses
- Caching responses
- Timeouts
- Retry policies
- Filtering and request validations
- Request pre-processing, response post-processing
- ... and more!

All of these capabilities can be bolted onto your existing Rust codebases that use the `solana-client` crate's `RpcClient`.

## How it Works
Tokio's Tower crate provides a [Service trait](https://tokio.rs/blog/2021-05-14-inventing-the-service-trait) that provides
a general abstraction over "Request-Response". 

This crate leverages this to create an implementation of the `RpcSender` trait, which can be used to construct
an `RpcClient`. From there, it's business as usual for client-side Solana interaction with Rust, with all of the
custom behavior bolted on!

This crate contains various services and layers which chain together to replicate the usual Solana `RpcClient` functionality.

On top of that, it's very easy to add additional behavior using layers from the Tower service ecosystem.

The Tower `ServiceBuilder` provides the easiest means of adding some of the most common behaviors.
This crate re-exports `ServiceBuilder` as `RpcClientBuilder`.

Here's a simple example showing how to add rate-limiting to an RpcClient. See the `examples/` directory for more.

```rust
use std::time::Duration;

use solana_devtools_rpc::prelude::*;

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
```

By default, the HTTP client behaves the same as the vanilla Solana RPC client.
So you'll get the usual error parsing and 429 retries out of the box.
The behavior is broken down into layers which are automatically added when you call `.http(url)` on the builder. The 429 retry behavior can be switched off.

### Order Matters
When you add layers to a service, order matters.
So, let's say you construct something like the snippet below:
```rust
use solana_devtools_rpc::prelude::*;

let _ = ServiceBuilder::new()
    .layer_a()
    .layer_b()
    .layer_c()
    .http(url)
    .build_rpc_client();
```
The request / response processing would be ordered like this: `a --> b --> c --> internet --> c --> b --> a`.