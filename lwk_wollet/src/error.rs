use elements::OutPoint;

/// Error type for the whole crate.
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
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

    #[error("Hex Error: {0}")]
    ElementsHex(crate::elements::hex::Error),

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
    SecpZkpError(#[from] crate::elements::secp256k1_zkp::Error),

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

    #[error("Address must be explicit")]
    NotExplicitAddress,

    #[error("Address must be confidential")]
    NotConfidentialAddress,

    #[error("Input must be confidential")]
    NotConfidentialInput,

    #[error("Insufficient funds: missing {missing_sats} units for {} {asset_id}",
        .is_token.then(|| "reissuance token").unwrap_or("asset"))]
    InsufficientFunds {
        missing_sats: u64,
        asset_id: crate::elements::AssetId,
        is_token: bool,
    },

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
    Url(#[from] crate::UrlError),

    #[error("Manual coin selection is not allowed when assets are involved (this limitation will be removed in the future)")]
    ManualCoinSelectionOnlyLbtc,

    #[error("Missing wallet UTXO {0}")]
    MissingWalletUtxo(OutPoint),

    #[error("Transaction has empty witness, did you forget to sign and finalize?")]
    EmptyWitness,

    #[error(transparent)]
    LiquidexError(#[from] crate::liquidex::LiquidexError),

    #[error("Issuance amount greater than 21M*10^8 are not allowed")]
    IssuanceAmountGreaterThanBtcMax,

    #[error("Number of transaction inputs ({0}) exceeds maximum allowed input count of 256")]
    TooManyInputs(usize),

    #[error("Cannot use derivation index when the descriptor has no wildcard")]
    IndexWithoutWildcard,

    #[error("Given contract does not commit to asset '{0}'")]
    InvalidContractForAsset(String),

    #[error("Given transaction does not contain issuance of asset '{0}'")]
    InvalidIssuanceTxtForAsset(String),

    #[cfg(feature = "test_wallet")]
    #[error(transparent)]
    SignerError(#[from] lwk_signer::NewError),

    #[cfg(feature = "amp0")]
    #[error(transparent)]
    RmpvDecodeError(#[from] rmpv::decode::Error),

    #[cfg(feature = "amp0")]
    #[error(transparent)]
    RmpvEncodeError(#[from] rmpv::encode::Error),

    #[cfg(feature = "amp0")]
    #[error(transparent)]
    RmpvExtError(#[from] rmpv::ext::Error),

    #[cfg(feature = "amp0")]
    #[error(transparent)]
    RmpSerdeDecodeError(#[from] rmp_serde::decode::Error),

    #[cfg(feature = "amp0")]
    #[error(transparent)]
    RmpSerdeEncodeError(#[from] rmp_serde::encode::Error),

    #[cfg(feature = "amp0")]
    #[error("Cannot generate address for AMP0 wallets using this call, use Amp0::address()")]
    Amp0AddressError,
}

// cannot derive automatically with this error because of trait bound
impl From<aes_gcm_siv::aead::Error> for Error {
    fn from(err: aes_gcm_siv::aead::Error) -> Self {
        Self::Aes(err.to_string())
    }
}

impl From<elements::hex::Error> for Error {
    fn from(err: elements::hex::Error) -> Self {
        Self::ElementsHex(err)
    }
}
