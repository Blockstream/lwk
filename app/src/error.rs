use tiny_jrpc::error::ImplementationDefinedCode;

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

impl Error {
    pub fn as_impl_defined_code(&self) -> ImplementationDefinedCode {
        match self {
            Error::Jade(_) => ImplementationDefinedCode::new(-32_013).unwrap(),
            Error::Wollet(_) => ImplementationDefinedCode::new(-32_005).unwrap(),
            Error::SignerNew(_) => ImplementationDefinedCode::new(-32_006).unwrap(),
            Error::Signer(_) => ImplementationDefinedCode::new(-32_007).unwrap(),
            Error::WalletNotExist(_) => ImplementationDefinedCode::new(-32_008).unwrap(),
            Error::WalletAlreadyLoaded(_) => ImplementationDefinedCode::new(-32_009).unwrap(),
            Error::SignerNotExist(_) => ImplementationDefinedCode::new(-32_010).unwrap(),
            Error::SignerAlreadyLoaded(_) => ImplementationDefinedCode::new(-32_011).unwrap(),
            _ => tiny_jrpc::error::GENERIC,
        }
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::Generic(message)
    }
}

impl From<Error> for tiny_jrpc::error::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Stop => tiny_jrpc::error::Error::Stop,
            e => tiny_jrpc::error::Error::new_implementation_defined(&e, e.as_impl_defined_code()),
        }
    }
}
