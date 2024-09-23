use reqwest::Url;
use solana_rpc_tower::rpc_sender_impl::RpcClientSender;

#[tokio::main]
async fn main() {
    let client =
        RpcClientSender::new_http(Url::try_from("https://api.mainnet-beta.solana.com").unwrap())
            .into_rpc_client(None);
    let _ = client.get_version().await;
}
