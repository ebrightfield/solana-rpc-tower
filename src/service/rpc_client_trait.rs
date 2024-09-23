// use std::future::Future;

// use serde_json::Value;
// use solana_client::client_error::Result as ClientResult;
// use solana_sdk::signature::Signature;
// use tower::{BoxError, Service};

// use super::rpc_sender_impl::SolanaClientRequest;

// #[async_trait::async_trait]
// pub trait RpcSenderExt {
//     fn confirm_transaction(&mut self, signature: &Signature) -> ClientResult<bool>;
//     // Get transaction
//     // Get recent blockash
// }

// impl<T> RpcSenderExt for T
// where
//     T: Service<SolanaClientRequest, Response = Value, Error = BoxError> + Send + 'static,
//     T::Future: Future<Output = Result<Value, BoxError>> + Send + 'static,
// {
//     // In general, we want to be converting these requests into something that the service T can handle
//     fn confirm_transaction(&mut self, signature: &Signature) -> ClientResult<bool> {
//         todo!()
//     }
// }

// TODO Identical methods to RPC Client, as a trait, implemented for S: SolanaRpcClientService
