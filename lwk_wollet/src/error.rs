use elements::OutPoint;

/// Error type for the whole crate.
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error("{0}")]
    Generic(String),

    #[error("{url} returned HTTP {status}{}", .body.as_deref().map(|b| format!(": {b}")).unwrap_or_default())]
    EsploraHttpError {
        url: String,
        status: u16,
        body: Option<String>,
    },

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

    #[cfg(feature = "reqwest")]
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[cfg(feature = "amp2")]
    #[error("AMP2 server at {url} returned HTTP {status}{}", .body.as_deref().map(|b| format!(": {b}")).unwrap_or_default())]
    Amp2HttpError {
        url: String,
        status: u16,
        body: Option<String>,
    },

    #[error("Address must be explicit")]
    NotExplicitAddress,

    #[error("Address must be confidential")]
    NotConfidentialAddress,

    #[error("Input must be confidential")]
    NotConfidentialInput,

    #[error("Insufficient funds: missing {missing_sats} units for asset {asset_id}")]
    InsufficientFunds {
        missing_sats: u64,
        asset_id: crate::elements::AssetId,
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

    #[error("Unsupported pre-segwit descriptor")]
    UnsupportedDescriptorPreSegwit,

    #[error("Unsupported taproot descriptor")]
    UnsupportedDescriptorTaproot,

    #[error("Descriptor not supported: unknown segwit version")]
    UnsupportedDescriptorSegwitUnknownVersion,

    #[error("Missing PSET")]
    MissingPset,

    #[error("Send many cannot be called with an empty addressee list")]
    SendManyEmptyAddressee,

    #[error("Private blinding key not available")]
    MissingPrivateBlindingKey,

    #[error("Contract does not commit to asset id")]
    ContractDoesNotCommitToAssetId,

    #[error("Update height {update_tip_height} too old (internal height {cache_tip_height})")]
    UpdateHeightTooOld {
        update_tip_height: u32,
        cache_tip_height: u32,
    },

    #[error("Update created on a wallet with status {update_status} while current wallet has {wollet_status}")]
    UpdateOnDifferentStatus {
        wollet_status: u64,
        update_status: u64,
    },

    #[error("Issuance and reissuance are mutually exclusive")]
    IssuanceReissuanceMutuallyExclusive,

    // TODO: remove once reissuance supports multiple calls like issuance does.
    #[error("A reissuance has already been set on this transaction")]
    ReissuanceAlreadySet,

    #[error("Cannot mix pinned and non-pinned issuances in the same transaction")]
    IssuanceModesMixing,

    #[error("More issuances than inputs")]
    IssuanceInputCountMismatch,

    #[error("Issuance pinned to outpoint {0} not present in the manual inputs order")]
    IssuanceOutpointNotInInputsOrder(OutPoint),

    #[error("Pinning issuance to input requires manual inputs order")]
    IssuancePinRequiresInputsOrder,

    #[error("Blockchain backend have not implemented waterfalls method")]
    WaterfallsUnimplemented,

    #[error("Cannot use waterfalls scan with elip151 because it would reveal the blinding key to the server")]
    UsingWaterfallsWithElip151,

    #[error("Cannot encrypt")]
    CannotEncrypt,

    #[error("Cannot parse server recipient key")]
    CannotParseRecipientKey,

    #[cfg(any(feature = "electrum", feature = "amp2"))]
    #[error(transparent)]
    Url(#[from] UrlError),

    #[error("Manual coin selection is not allowed when assets are involved (this limitation will be removed in the future)")]
    ManualCoinSelectionOnlyLbtc,

    #[error("Missing wallet UTXO {0}")]
    MissingWalletUtxo(OutPoint),

    #[error("Duplicated outpoint {0} in {1}")]
    DuplicatedOutpoint(OutPoint, String),

    #[error("Manual inputs order requires `set_wallet_utxos` to be set too")]
    InputsOrderRequiresWalletUtxos,

    #[error(
        "Manual inputs order must be exactly the union of the outpoints passed to `set_wallet_utxos` and the external utxos"
    )]
    InputsOrderUtxosMismatch,

    #[error("Reissuance token {0} utxo is required but not present in the manual inputs order")]
    TokenUtxoNotInInputsOrder(crate::elements::AssetId),

    #[error("Reissuance token {0} utxo not found in the wallet")]
    MissingReissuanceTokenUtxo(crate::elements::AssetId),

    #[error("Manual inputs order requires issuances to be pinned to inputs")]
    InputsOrderRequiresPinnedIssuance,

    #[error("LiquiDEX make/take is not supported together with a manual inputs order")]
    LiquidexUnsupportedWithInputsOrder,

    #[error("Transaction has empty witness, did you forget to sign and finalize?")]
    EmptyWitness,

    #[error(transparent)]
    LiquidexError(#[from] crate::liquidex::LiquidexError),

    #[error("Store error: {0}")]
    StoreError(lwk_common::BoxError),

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

    #[error("Unsupported (wollet does not have CT descriptor)")]
    UnsupportedWithoutDescriptor,

    #[error("Invalid SPK format: expected 'blinding_key_hex:script_pubkey_hex'")]
    InvalidSpkFormat,

    #[error("Index out of range")]
    IndexOutOfRange,

    #[error(
        "Wollet and client are incompatible: they must be both 'utxo_only' or both non-'utxo_only'"
    )]
    UtxoOnlyIncompatible,

    #[error("Cannot access browser window for async sleep")]
    AsyncSleepMissingWindow,

    #[error("Async sleep failed: {0}")]
    AsyncSleepFailed(String),
}

// cannot derive automatically with this error because of trait bound
impl From<aes_gcm_siv::aead::Error> for Error {
    fn from(err: aes_gcm_siv::aead::Error) -> Self {
        Self::Aes(err.to_string())
    }
}

impl From<lwk_common::EncryptError> for Error {
    fn from(err: lwk_common::EncryptError) -> Self {
        match err {
            lwk_common::EncryptError::MissingNonce => {
                Self::Generic("Missing nonce in encrypted bytes".to_string())
            }
            lwk_common::EncryptError::Aead(err) => Self::Aes(err),
        }
    }
}

impl From<elements::hex::Error> for Error {
    fn from(err: elements::hex::Error) -> Self {
        Self::ElementsHex(err)
    }
}

/// Error type when parsing a string to the [`url::Url`] type.
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum UrlError {
    #[error(transparent)]
    Url(#[from] url::ParseError),

    #[error("Invalid schema `{0}` supported ones are `ssl` or `tcp`")]
    Schema(String),

    #[error("Port is missing")]
    MissingPort,

    #[error("Domain is missing")]
    MissingDomain,

    #[error("Cannot specify `ssl` scheme without a domain")]
    SslWithoutDomain,

    #[error("Cannot validate the domain without tls")]
    ValidateWithoutTls,

    #[error("Don't specify the scheme in the url")]
    NoScheme,
}
