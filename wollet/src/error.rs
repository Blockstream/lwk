#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Generic(String),

    #[error("could not parse SocketAddr `{0}`")]
    AddrParse(String),

    #[error("Aes {0}")]
    Aes(String),

    #[error("Blinding bare key unsupported")]
    BlindingBareUnsupported,

    #[error(transparent)]
    InvalidMnemonic(#[from] bip39::Error),

    #[error(transparent)]
    Bitcoin(#[from] elements_miniscript::elements::bitcoin::Error),

    #[error(transparent)]
    BitcoinHashes(#[from] elements_miniscript::elements::bitcoin::hashes::error::Error),

    #[error(transparent)]
    BitcoinBIP32Error(#[from] elements_miniscript::elements::bitcoin::bip32::Error),

    #[error(transparent)]
    BitcoinConsensus(#[from] elements_miniscript::elements::bitcoin::consensus::encode::Error),

    #[error(transparent)]
    JsonFrom(#[from] serde_json::Error),

    #[error(transparent)]
    StdIOError(#[from] std::io::Error),

    #[error(transparent)]
    ClientError(#[from] electrum_client::Error),

    #[error(transparent)]
    SliceConversionError(#[from] std::array::TryFromSliceError),

    #[error(transparent)]
    ElementsEncode(#[from] elements_miniscript::elements::encode::Error),

    #[error(transparent)]
    ElementsPset(#[from] elements_miniscript::elements::pset::Error),

    #[error(transparent)]
    PsetBlindError(#[from] elements_miniscript::elements::pset::PsetBlindError),

    #[error(transparent)]
    Send(#[from] std::sync::mpsc::SendError<()>),

    #[error(transparent)]
    Secp256k1(#[from] elements_miniscript::elements::bitcoin::secp256k1::Error),

    #[error(transparent)]
    Secp256k1Zkp(#[from] elements_miniscript::elements::secp256k1_zkp::Error),

    #[error(transparent)]
    HexBitcoinHashes(#[from] elements_miniscript::elements::bitcoin::hashes::hex::Error),

    #[error(transparent)]
    DeserializeCBORError(#[from] ciborium::de::Error<std::io::Error>),

    #[error(transparent)]
    SerializeCBORError(#[from] ciborium::ser::Error<std::io::Error>),

    #[error(transparent)]
    ElementsMiniscript(#[from] elements_miniscript::Error),

    #[error(transparent)]
    DescConversion(#[from] elements_miniscript::descriptor::ConversionError),

    #[error(transparent)]
    Unblind(#[from] elements_miniscript::elements::UnblindError),

    #[error(transparent)]
    AddressError(#[from] elements_miniscript::elements::AddressError),

    #[error(transparent)]
    PsetDetailsError(#[from] crate::pset_details::Error),

    #[error("Address must be confidential")]
    NotConfidentialAddress,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Missing transaction")]
    MissingTransaction,

    #[error("Missing vout")]
    MissingVout,

    #[error("Invalid amount")]
    InvalidAmount,

    #[error("The script is not owned by this wallet")]
    ScriptNotMine,
}

// cannot derive automatically with this error because of trait bound
impl From<aes_gcm_siv::aead::Error> for Error {
    fn from(err: aes_gcm_siv::aead::Error) -> Self {
        Self::Aes(err.to_string())
    }
}
