#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Generic(String),

    #[error("Aes {0}")]
    Aes(String),

    #[error("Blinding bare key unsupported")]
    BlindingBareUnsupported,

    #[error("Blinding view key with wildcard unsupported")]
    BlindingViewWildcardUnsupported,

    #[error("Blinding view key with multipath unsupported")]
    BlindingViewMultiUnsupported,

    #[error(transparent)]
    BitcoinBIP32Error(#[from] crate::bitcoin::bip32::Error),

    #[error(transparent)]
    JsonFrom(#[from] serde_json::Error),

    #[error(transparent)]
    StdIOError(#[from] std::io::Error),

    #[cfg(feature = "electrum")]
    #[error(transparent)]
    ClientError(#[from] electrum_client::Error),

    #[cfg(feature = "elements_rpc")]
    #[error(transparent)]
    ElementsRpcError(#[from] bitcoincore_rpc::Error),

    #[cfg(feature = "elements_rpc")]
    #[error("Elements RPC returned an unexpected value for call {0}")]
    ElementsRpcUnexpectedReturn(String),

    #[error(transparent)]
    ElementsEncode(#[from] crate::elements::encode::Error),

    #[error(transparent)]
    Hashes(#[from] crate::elements::hashes::FromSliceError),

    #[error(transparent)]
    ElementsPset(#[from] crate::elements::pset::Error),

    #[error(transparent)]
    ElementsPsetParse(#[from] crate::elements::pset::ParseError),

    #[error(transparent)]
    PsetBlindError(#[from] crate::elements::pset::PsetBlindError),

    #[error(transparent)]
    Secp256k1(#[from] crate::secp256k1::Error),

    #[error(transparent)]
    HexToBytesError(#[from] crate::hashes::hex::HexToBytesError),

    #[error(transparent)]
    HexToArrayError(#[from] crate::hashes::hex::HexToArrayError),

    #[error(transparent)]
    ElementsMiniscript(#[from] elements_miniscript::Error),

    #[error(transparent)]
    ElementsMiniscriptPset(#[from] elements_miniscript::psbt::Error),

    #[error(transparent)]
    DescConversion(#[from] elements_miniscript::descriptor::ConversionError),

    #[error(transparent)]
    Unblind(#[from] crate::elements::UnblindError),

    #[error(transparent)]
    AddressError(#[from] crate::elements::AddressError),

    #[error(transparent)]
    PsetDetailsError(#[from] lwk_common::Error),

    #[error(transparent)]
    InvalidKeyOriginXpubError(#[from] lwk_common::InvalidKeyOriginXpub),

    #[error(transparent)]
    UtxoUpdateError(#[from] elements_miniscript::psbt::UtxoUpdateError),

    #[error(transparent)]
    OutputUpdateError(#[from] elements_miniscript::psbt::OutputUpdateError),

    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),

    #[cfg(feature = "esplora")]
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    PersistError(#[from] crate::persister::PersistError),

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

    #[error("Missing keyorigin")]
    MissingKeyorigin,

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

    #[error("Descriptor without wildcard not supported")]
    UnsupportedDescriptorWithoutWildcard,

    #[error(
        "Multipath descriptor must have only the external/internal multipath (eg '.../<0;1>/*')"
    )]
    UnsupportedMultipathDescriptor,

    #[error("Descriptor with segwit not v0 is not supported")]
    UnsupportedDescriptorNonV0, // TODO add non supported descriptor type as field or split it further: UnsupportedDescriptorPreSegwit, UnsupportedDescriptorTaproot, UnsupportedDescriptorUnknownSegwitVersion

    #[error("Missing PSET")]
    MissingPset,

    #[error("Send many cannot be called with an empty addressee list")]
    SendManyEmptyAddressee,

    #[error("Private blinding key not available")]
    MissingPrivateBlindingKey,

    #[error("Contract does not commit to asset id")]
    ContractDoesNotCommitToAssetId,

    #[error("Update height {update_tip_height} too old (internal height {store_tip_height})")]
    UpdateHeightTooOld {
        update_tip_height: u32,
        store_tip_height: u32,
    },

    #[error("Update created on a wallet with status {update_status} while current wallet has {wollet_status}")]
    UpdateOnDifferentStatus {
        wollet_status: u64,
        update_status: u64,
    },

    #[error("An issuance has already being set on this tx builder")]
    IssuanceAlreadySet,

    #[error("Blockchain backend have not implemented waterfalls method")]
    WaterfallsUnimplemented,

    #[error("Cannot use waterfalls scan with elip151 because it would reveal the blinding key to the server")]
    UsingWaterfallsWithElip151,

    #[error("Cannot encrypt")]
    CannotEncrypt,

    #[error("Cannot parse server recipient key")]
    CannotParseRecipientKey,

    #[cfg(feature = "electrum")]
    #[error(transparent)]
    Url(#[from] crate::clients::blocking::electrum_client::UrlError),
}

// cannot derive automatically with this error because of trait bound
impl From<aes_gcm_siv::aead::Error> for Error {
    fn from(err: aes_gcm_siv::aead::Error) -> Self {
        Self::Aes(err.to_string())
    }
}
