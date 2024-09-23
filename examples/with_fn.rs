use futures::future::BoxFuture;
use serde_json::Value;
use solana_client::{
    client_error::{ClientError, ClientErrorKind},
    rpc_response::{Response, RpcResponseContext, RpcVersionInfo},
};
use solana_rpc_tower::{
    builder::ServiceBuilderExt, rpc_sender_impl::SolanaClientRequest, RpcRequest,
};
use tower::{BoxError, ServiceBuilder};

#[tokio::main]
async fn main() {
    // We can inline an async closure
    let _client = ServiceBuilder::new()
        .with_fn(|req| async move { Ok(req.1) })
        .build_rpc_client();

    // Or we can pass in a much more involved routine.
    let client = ServiceBuilder::new()
        .with_fn(fake_service)
        .build_rpc_client();
    let _ = client.get_version().await;
}

// You can also use standalone functions
fn fake_service(request: SolanaClientRequest) -> BoxFuture<'static, Result<Value, BoxError>> {
    Box::pin(async move {
        let (method, params) = request;
        match method {
            RpcRequest::GetBalance => {
                tracing::info!(?params);
                let resp = serde_json::to_value(Response {
                    context: RpcResponseContext {
                        slot: 100,
                        api_version: None,
                    },
                    value: 123456789,
                })
                .unwrap();
                tracing::info!(?resp);
                Ok(resp)
            }
            RpcRequest::GetVersion => Ok(serde_json::to_value(RpcVersionInfo {
                solana_core: "1.18.21".to_string(),
                feature_set: Some(99),
            })
            .unwrap()),
            _ => Err(Box::new(ClientError::new_with_request(
                ClientErrorKind::Custom("foo".to_string()),
                method,
            )) as BoxError),
        }
    })
}
