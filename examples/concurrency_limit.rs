use reqwest::Url;
use solana_rpc_tower::prelude::*;
use solana_sdk::signature::Signature;
use solana_transaction_status::UiTransactionEncoding;

#[tokio::main]
async fn main() {
    // This example adds a concurrency limit to the client, throttling the client
    // to a limit of `n` means the service to block whenever there are already `n` in-flight requests.
    let client = RpcClientBuilder::new()
        .concurrency_limit(2)
        .http(Url::try_from("https://api.mainnet-beta.solana.com").unwrap())
        .build_rpc_client();
    let _ = client.get_version().await;

    // Combined with the `AndThen` layer, we can very easily configure a processing pipeline
    // that maintains a maximum number of concurrent processing jobs.

    let signatures = vec![
        Signature::new_unique(),
        Signature::new_unique(),
        Signature::new_unique(),
        Signature::new_unique(),
        Signature::new_unique(),
        // ... lots of signatures
    ];

    let client = RpcClientBuilder::new()
        .and_then(|response| async move {
            // ... do something with the response
            Ok(response)
        })
        .concurrency_limit(2)
        .http(Url::try_from("https://api.mainnet-beta.solana.com").unwrap())
        .build_rpc_client();
    for signature in signatures {
        let _ = client
            .get_transaction(&signature, UiTransactionEncoding::Json)
            .await;
    }
}
