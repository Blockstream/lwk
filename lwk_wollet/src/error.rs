use elements::{AssetId, OutPoint};

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

    /// An authenticated backend denied the request because it lacks a valid token
    /// (esplora/waterfalls HTTP 401, electrum proxy JSON-RPC -32004).
    #[error("authentication required")]
    AuthenticationRequired,

    /// An authenticated backend denied the request because the account is out of credits
    /// (esplora/waterfalls HTTP 402, electrum proxy JSON-RPC -32000).
    #[error("insufficient credits")]
    InsufficientCredits,

    /// An authenticated backend denied the request because it is rate limited
    /// (esplora/waterfalls HTTP 429, electrum proxy JSON-RPC -32002/-32003).
    #[error("rate limited")]
    RateLimited,

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
    ClientError(electrum_client::Error),

    #[cfg(feature = "elements_rpc")]
    #[error(transparent)]
    ElementsRpcError(#[from] bitcoincore_rpc::Error),

    #[cfg(feature = "elements_rpc")]
    #[error("Elements RPC returned an unexpected value for call {0}")]
    ElementsRpcUnexpectedReturn(String),

    #[error(transparent)]
    ElementsEncode(#[from] crate::elements::encode::Error),

    #[error(transparent)]
    BitcoinEncode(#[from] crate::bitcoin::consensus::encode::Error),

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

    #[error("Invalid federation peg: {0}")]
    InvalidFedPeg(String),

    #[error("Pegin claim script must be a native segwit v0 wallet script")]
    UnsupportedPeginClaimScript,

    #[error("Federation peg network {fed_peg} does not match wallet network {wallet}")]
    PeginNetworkMismatch {
        wallet: &'static str,
        fed_peg: &'static str,
    },

    #[error("Pegin transaction does not pay the expected address")]
    PeginOutputNotFound,

    #[error("Pegin transaction pays the expected address more than once")]
    PeginOutputAmbiguous,

    #[error("Pegin transaction output index {vout} does not fit in an outpoint")]
    PeginOutputIndexOverflow { vout: usize },

    #[error("Pegin transaction output index {vout} conflicts with Elements input flags")]
    PeginVoutConflictsWithFlags { vout: u32 },

    #[error("Invalid pegin txout proof: {0}")]
    InvalidPeginProof(String),

    #[error("Pegin transaction {txid} is not included in the txout proof")]
    PeginTransactionNotInProof { txid: crate::bitcoin::Txid },

    #[error("Pegin inputs cannot be used with {0}")]
    PeginUnsupportedBuilderMode(&'static str),

    #[error("Missing PSET")]
    MissingPset,

    #[error("Send many cannot be called with an empty addressee list")]
    SendManyEmptyAddressee,

    #[error("Private blinding key not available")]
    MissingPrivateBlindingKey,

    #[error("The transaction has confidential inputs but no output to blind")]
    MissingBlindedOutput,

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

    #[error("Cannot mix pinned and non-pinned issuances in the same transaction")]
    IssuanceModesMixing,

    #[error("More issuances than inputs")]
    IssuanceInputCountMismatch,

    #[error("Issuance pinned to outpoint {0} not present in the manual inputs order")]
    IssuanceOutpointNotInInputsOrder(OutPoint),

    #[error("Pinning issuance to input requires manual inputs order")]
    IssuancePinRequiresInputsOrder,

    #[error("The (re)issuance outputs sum up to {found} satoshi, but {expected} are (re)issued")]
    IssuanceOutputsAmountMismatch { expected: u64, found: u64 },

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

    #[error("Duplicated reissued asset {0}")]
    DuplicatedReissuanceAsset(AssetId),

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

    #[error("Reissuance pinned to outpoint {0} not present in the manual inputs order")]
    ReissuanceOutpointNotInInputsOrder(OutPoint),

    #[error("Reissuance pinned to outpoint {outpoint} not holding the reissuance token {token}")]
    ReissuancePinnedInputNotToken {
        outpoint: OutPoint,
        token: crate::elements::AssetId,
    },

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

    #[cfg(feature = "amp0")]
    #[error("Invalid login challenge received from the server")]
    Amp0InvalidChallenge,

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

    #[error("Invalid network")]
    InvalidNetwork,

    #[error("PSET validation failed: {0}")]
    PsetValidationError(#[from] lwk_common::PsetValidationError),

    #[cfg(feature = "amp2")]
    #[error("AMP2 cosign didn't add any signatures")]
    Amp2NoSigsAdded,
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

/// The `error.data.source` value the Blockstream Electrum RPC proxy stamps on the denials it
/// owns, so its JSON-RPC codes can be told apart from another server's use of the same numbers.
#[cfg(feature = "electrum")]
const ELECTRUM_PROXY_SOURCE: &str = "electrs-electrum-proxy";

/// Maps the Blockstream Electrum RPC proxy's JSON-RPC denial codes to the common [`Error`] denial
/// variants, so callers handle them the same way as the esplora/waterfalls HTTP denials. Returns
/// `None` for any other electrum error.
///
/// The codes live in the JSON-RPC implementation-defined range (-32000..-32099), which is not
/// globally unique: any other Electrum/JSON-RPC server assigns them different meanings (our own
/// `lwk_tiny_jrpc`, for instance, uses -32000/-32002/-32004 for unrelated errors). So the mapping
/// keys on the proxy-owned `error.data.source` marker rather than the code alone; without that
/// marker the error is left untouched. A denial can arrive nested inside
/// [`electrum_client::Error::AllAttemptsErrored`] (a reconnect re-runs the token-carrying
/// handshake and is denied), so this recurses into it.
#[cfg(feature = "electrum")]
pub(crate) fn electrum_denial_variant(error: &electrum_client::Error) -> Option<Error> {
    match error {
        electrum_client::Error::Protocol(value) => {
            let from_proxy = value
                .get("data")
                .and_then(|d| d.get("source"))
                .and_then(|s| s.as_str())
                == Some(ELECTRUM_PROXY_SOURCE);
            if !from_proxy {
                return None;
            }
            match value.get("code").and_then(|c| c.as_i64()) {
                Some(-32004) => Some(Error::AuthenticationRequired),
                Some(-32000) => Some(Error::InsufficientCredits),
                Some(-32002) | Some(-32003) => Some(Error::RateLimited),
                _ => None,
            }
        }
        electrum_client::Error::AllAttemptsErrored(errors) => {
            errors.iter().find_map(electrum_denial_variant)
        }
        _ => None,
    }
}

// The proxy-owned `data.source` marker makes the denial codes safe to map globally (a non-proxy
// server lacks the marker), so the blanket conversion maps them; everything else stays ClientError.
#[cfg(feature = "electrum")]
impl From<electrum_client::Error> for Error {
    fn from(err: electrum_client::Error) -> Self {
        electrum_denial_variant(&err).unwrap_or(Error::ClientError(err))
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
