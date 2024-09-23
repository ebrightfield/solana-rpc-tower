use reqwest::Url;
use solana_rpc_tower::prelude::*;

#[tokio::main]
async fn main() {
    // The tower `ServiceBuilder` allows one to design pipelines for request and response processing.
    // This simple example intercepts the response with an `AndThen` layer.
    // This example doesn't do much, but it shows the basic process by which we are able to
    // add all kinds of behavior to our RPC client.
    let client = RpcClientBuilder::new()
        .and_then(|resp| async move {
            println!("response: {:#?}", resp);
            // Do something with the response here
            Ok(resp)
        })
        .http(Url::try_from("https://api.mainnet-beta.solana.com").unwrap())
        .build_rpc_client();
    let _ = client.get_version().await;
}
