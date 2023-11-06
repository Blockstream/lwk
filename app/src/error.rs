#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Tiny HTTP Error: {0}")]
    TinyHttp(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("JSON RPC HTTP Client Error: {0}")]
    JsonRpcHttp(#[from] jsonrpc::simple_http::Error),
    #[error("JSON RPC Client Error: {0}")]
    JsonRpcClient(#[from] jsonrpc::Error),
    #[error("Serde JSON Error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Jade Error: {0}")]
    Jade(#[from] jade::Error),
    #[error("Jade Unlock Error: {0}")]
    Unlock(#[from] jade::unlock::Error),

    #[error("Trying to start an already started server")]
    AlreadyStarted,

    #[error("Trying to join a non started server")]
    NotStarted,
}
