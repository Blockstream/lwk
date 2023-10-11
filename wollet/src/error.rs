#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Generic(String),

    #[error("Aes {0}")]
    Aes(String),

    #[error("Blinding bare key unsupported")]
    BlindingBareUnsupported,

    #[error(transparent)]
    BitcoinBIP32Error(#[from] crate::bitcoin::bip32::Error),

    #[error(transparent)]
    JsonFrom(#[from] serde_json::Error),

    #[error(transparent)]
    StdIOError(#[from] std::io::Error),

    #[error(transparent)]
    ClientError(#[from] electrum_client::Error),

    #[error(transparent)]
    ElementsEncode(#[from] crate::elements::encode::Error),

    #[error(transparent)]
    ElementsPset(#[from] crate::elements::pset::Error),

    #[error(transparent)]
    PsetBlindError(#[from] crate::elements::pset::PsetBlindError),

    #[error(transparent)]
    Secp256k1(#[from] crate::secp256k1::Error),

    #[error(transparent)]
    HexBitcoinHashes(#[from] crate::hashes::hex::Error),

    #[error(transparent)]
    DeserializeCBORError(#[from] ciborium::de::Error<std::io::Error>),

    #[error(transparent)]
    SerializeCBORError(#[from] ciborium::ser::Error<std::io::Error>),

    #[error(transparent)]
    ElementsMiniscript(#[from] elements_miniscript::Error),

    #[error(transparent)]
    DescConversion(#[from] elements_miniscript::descriptor::ConversionError),

    #[error(transparent)]
    Unblind(#[from] crate::elements::UnblindError),

    #[error(transparent)]
    AddressError(#[from] crate::elements::AddressError),

    #[error(transparent)]
    PsetDetailsError(#[from] crate::pset_details::Error),

    #[error(transparent)]
    UtxoUpdateError(#[from] elements_miniscript::psbt::UtxoUpdateError),

    #[error("Address must be confidential")]
    NotConfidentialAddress,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Missing issuance")]
    MissingIssuance,

    #[error("Missing transaction")]
    MissingTransaction,

    #[error("Missing vin")]
    MissingVin,

    #[error("Missing vout")]
    MissingVout,

    #[error("Invalid amount")]
    InvalidAmount,

    #[error("The script is not owned by this wallet")]
    ScriptNotMine,

    #[error("Invalid domain")]
    InvalidDomain,

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

    #[error("Descriptor not supported")]
    UnsupportedDescriptor,

    #[error("Missing PSET")]
    MissingPset,

    #[error("Send many cannot be called with an empty addressee list")]
    SendManyEmptyAddressee,

    #[error("Private blinding key not available")]
    MissingPrivateBlindingKey,
}

// cannot derive automatically with this error because of trait bound
impl From<aes_gcm_siv::aead::Error> for Error {
    fn from(err: aes_gcm_siv::aead::Error) -> Self {
        Self::Aes(err.to_string())
    }
}
