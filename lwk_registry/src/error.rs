#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Generic(String),

    #[error("Downloaded transaction txid does not match the requested txid")]
    TxidMismatch,

    #[cfg(feature = "client")]
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error("Invalid domain")]
    InvalidDomain,

    #[error("Contract does not commit to asset id")]
    ContractDoesNotCommitToAssetId,

    #[error(transparent)]
    JsonFrom(#[from] serde_json::Error),

    #[error("Invalid version")]
    InvalidVersion,

    #[error("Invalid precision")]
    InvalidPrecision,

    #[error("Invalid name")]
    InvalidName,

    #[error("Invalid ticker")]
    InvalidTicker,

    #[error("Invalid issuer pubkey")]
    InvalidIssuerPubkey,

    #[error(transparent)]
    StdIOError(#[from] std::io::Error),

    #[error(transparent)]
    Secp256k1(#[from] elements::bitcoin::secp256k1::Error),

    #[error("Given contract does not commit to asset '{0}'")]
    InvalidContractForAsset(String),

    #[error("Given transaction does not contain issuance of asset '{0}'")]
    InvalidIssuanceTxtForAsset(String),
}
