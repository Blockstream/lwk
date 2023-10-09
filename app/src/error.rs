#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Tiny HTTP Error: {0}")]
    TinyHttp(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("JSON RPC HTTP Client Error: {0}")]
    JsonRpcHttp(#[from] jsonrpc::simple_http::Error),
    #[error("JSON RPC Client Error: {0}")]
    JsonRpcClient(#[from] jsonrpc::Error),
}
