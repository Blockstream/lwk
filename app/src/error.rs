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

    #[error("Wollet Error: {0}")]
    Wollet(#[from] wollet::Error),

    #[error("Hex Error: {0}")]
    Hex(wollet::elements::hex::Error),

    #[error("Trying to start an already started server")]
    AlreadyStarted,

    #[error("Trying to join a non started server")]
    NotStarted,

    #[error("In the response received neither the result nor the error are set")]
    NeitherResultNorErrorSet,

    #[error("Rpc returned an error {0:?}")]
    RpcError(jsonrpc::error::RpcError),

    #[error("Signer New Error: {0}")]
    SignerNew(#[from] signer::NewError),

    #[error("Signer Error: {0}")]
    Signer(#[from] signer::SignerError),

    #[error("Wallet '{0}' does not exist")]
    WalletNotExist(String),

    #[error("Wallet '{0}' is already loaded")]
    WalletAlreadyLoaded(String),

    #[error("Signer '{0}' does not exist")]
    SignerNotExist(String),

    #[error("Signer '{0}' is already loaded")]
    SignerAlreadyLoaded(String),

    #[error("Received stop command")]
    Stop,

    // TODO remove into specific errors
    #[error("Generic error {0}")]
    Generic(String),
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Generic(message)
    }
}
impl From<wollet::elements::hex::Error> for Error {
    fn from(value: wollet::elements::hex::Error) -> Self {
        Error::Hex(value)
    }
}

impl From<Error> for tiny_jrpc::error::Error {
    fn from(value: Error) -> Self {
        // TODO map errors to specific implementation defined codes.
        // WOLLET_ERROR = -32_005,
        // SIGNER_NEW_ERROR = -32_006,
        // SIGNER_ERROR = -32_007,
        // WALLET_NOT_EXIST_ERROR = -32_008,
        // WALLET_ALREADY_LOADED = -32_009,
        // SIGNER_NOT_EXIST_ERROR = -32_010,
        // SIGNER_ALREADY_LOADED = -32_011,
        // JADE_ERROR = -32_013,
        match value {
            Error::Stop => tiny_jrpc::error::Error::Stop,
            e => tiny_jrpc::error::Error::new_implementation_defined(e, tiny_jrpc::error::GENERIC),
        }
    }
}
