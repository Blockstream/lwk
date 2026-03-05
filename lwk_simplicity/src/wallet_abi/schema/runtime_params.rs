//! Runtime transaction parameter schema used by `wallet-abi-0.1`.
//!
//! Serialization note:
//! enum variants are serialized in `snake_case` across this schema.

use crate::error::WalletAbiError;
use crate::taproot_pubkey_gen::TaprootPubkeyGen;
use crate::wallet_abi::schema::values::resolve_arguments;

use crate::scripts::{create_p2tr_address, load_program};

use lwk_wollet::bitcoin::XOnlyPublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::secp256k1_zkp::{PublicKey, SecretKey};
use lwk_wollet::elements::LockTime;
use lwk_wollet::elements::{AssetId, OutPoint, Script, Sequence};

use serde::{Deserialize, Serialize};

/// Top-level transaction-construction payload carried in `TxCreateRequest.params`.
///
/// This type is the request contract boundary between callers and runtime. It defines
/// declared inputs/outputs plus optional fee-rate and `lock_time` hints.
///
/// # Runtime behavior nuances
///
/// - Declared inputs/outputs are the starting point, not always the final transaction shape.
/// - Runtime may append auxiliary wallet inputs to close asset deficits.
/// - Runtime normalizes/appends fee output and may append per-asset change outputs.
/// - Input selection is deterministic for a fixed wallet snapshot and request, but state-aware:
///   declared input order can affect later selection and issuance-derived references.
///
/// # Security
///
/// Treat this payload as untrusted caller input:
/// - `provided` outpoints, blinding material, and all nested fields are trust boundaries.
/// - Avoid logging full raw payloads; nested fields can contain sensitive key material.
/// - Misconfigured fee rate or `lock_time` can produce surprising transaction behavior.
///
/// # UX guidance
///
/// - Keep declared input ordering stable when using issuance-linked `input_index` references.
/// - Do not assume requested outputs are the exact final output set; fee/change adjustments can occur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeParams {
    /// Declared contract inputs resolved in declaration order.
    ///
    /// Runtime resolves each entry sequentially and updates funding state after each step.
    /// This means input order can influence subsequent wallet candidate selection and
    /// issuance-derived asset id resolution.
    ///
    /// Runtime may append additional wallet inputs after declared-input resolution when
    /// declared inputs do not fully cover demand.
    ///
    /// Security note:
    /// nested source fields (especially caller-provided outpoints) are untrusted input.
    pub inputs: Vec<InputSchema>,
    /// Declared contract outputs materialized in declaration order.
    ///
    /// Output id `"fee"` has special meaning:
    /// runtime validates uniqueness/asset rules and then overwrites or appends the fee output
    /// to match the converged fee target.
    ///
    /// Runtime can append deterministic global change outputs (one per residual asset), so this
    /// list is not guaranteed to be the full final output set.
    pub outputs: Vec<OutputSchema>,
    /// Optional fee-rate override in sat/vB.
    ///
    /// When omitted, runtime uses its built-in default fee-rate policy.
    ///
    /// Runtime requires a finite non-negative value and rejects `NaN`, infinities, and negative
    /// values. Fee calculation is performed in a bounded fixed-point loop, so this field can
    /// influence both fee target and input-selection outcomes.
    ///
    /// UX note:
    /// zero is accepted but can lead to unexpected relay/broadcast outcomes depending on policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_rate_sat_vb: Option<f32>,
    /// Optional transaction fallback `lock_time`.
    ///
    /// Runtime writes this value to PSET `global.tx_data.fallback_locktime`.
    ///
    /// `lock_time` activation still depends on input sequence semantics. If all effective input
    /// sequences are final, `lock_time` is not consensus-active even when provided.
    ///
    /// UX note:
    /// callers should coordinate this with intended sequence values to avoid a false sense of
    /// timelock enforcement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_time: Option<LockTime>,
}

impl RuntimeParams {
    /// Decode request `params` JSON into [`RuntimeParams`].
    ///
    /// Contract guarantees:
    /// - rejects unknown top-level fields (`deny_unknown_fields`)
    /// - requires both `inputs` and `outputs` to be present
    /// - treats missing `fee_rate_sat_vb` / `lock_time` as `None`
    ///
    /// # Errors
    ///
    /// Returns [`WalletAbiError::InvalidRequest`] when JSON shape or field values are invalid.
    pub fn from_request_params(value: &serde_json::Value) -> Result<Self, WalletAbiError> {
        serde_json::from_value(value.clone())
            .map_err(|e| WalletAbiError::InvalidRequest(format!("invalid request params: {e}")))
    }

    /// Encode [`RuntimeParams`] into request `params` JSON.
    ///
    /// Serialization behavior:
    /// - always emits `inputs` and `outputs`
    /// - omits `fee_rate_sat_vb` and `lock_time` when they are `None`
    ///
    /// # Errors
    ///
    /// Returns [`WalletAbiError`] if JSON serialization fails.
    pub fn to_request_params_value(&self) -> Result<serde_json::Value, WalletAbiError> {
        serde_json::to_value(self).map_err(WalletAbiError::from)
    }
}

/// Asset constraint for wallet-sourced input selection (`UTXOSource::Wallet`).
///
/// # Runtime Semantics
///
/// During wallet candidate filtering:
/// - `none`: candidate passes regardless of asset id.
/// - `exact`: candidate passes only when
///   `candidate.unblinded.asset == asset_id`.
///
/// This predicate is conjunctive with [`AmountFilter`] and [`LockFilter`]:
/// candidates must satisfy all enabled dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssetFilter {
    /// No asset constraint.
    #[default]
    None,
    /// Require exact asset id equality.
    Exact {
        /// Expected asset id for candidate matching.
        asset_id: AssetId,
    },
}

/// Amount constraint for wallet-sourced input selection (`UTXOSource::Wallet`).
///
/// # Runtime Semantics
///
/// During wallet candidate filtering:
/// - `none`: candidate passes regardless of amount.
/// - `exact`: candidate passes only when
///   `candidate.unblinded.value == satoshi`.
/// - `min`: candidate passes only when
///   `candidate.unblinded.value >= satoshi`.
///
/// # Zero-Value Nuance
///
/// - `exact { satoshi: 0 }` accepts only zero-valued UTXOs.
/// - `min { satoshi: 0 }` behaves as an unconstraining lower bound for amount.
///
/// # UX Guidance
///
/// - Prefer `exact` when selecting a specific denomination/coin-sized input.
/// - Prefer `min` when selecting at-least funding candidates.
/// - Pair amount filters with [`AssetFilter::Exact`] for predictable selection
///   in multi-asset wallets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AmountFilter {
    /// No amount constraint.
    #[default]
    None,
    /// Require exact satoshi amount equality.
    Exact {
        /// Target satoshi amount.
        satoshi: u64,
    },
    /// Require satoshi amount at least this threshold.
    Min {
        /// Minimum satoshi threshold (inclusive).
        satoshi: u64,
    },
}

/// Locking-script constraint for wallet-sourced input selection (`UTXOSource::Wallet`).
///
/// # Runtime Semantics
///
/// During wallet candidate filtering:
/// - `none`: candidate passes regardless of script pubkey.
/// - `script`: candidate passes only on byte-for-byte equality with the wallet
///   snapshot `script_pubkey`.
///
/// # Pre-sync Note
///
/// Runtime may attempt a script pre-sync for `script`-locked filters before
/// resolution.
///
/// # Security and UX Guidance
///
/// - Treat script values in requests as untrusted input.
/// - Use a deterministic script source-of-truth (for example descriptor-derived
///   script generation) instead of manual edits to avoid silent mismatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LockFilter {
    /// No locking-script constraint.
    #[default]
    None,
    /// Require exact script-pubkey byte equality.
    Script {
        /// Expected script pubkey bytes.
        script: Script,
    },
}

/// Wallet-owned UTXO constraints used by `UTXOSource::Wallet`.
///
/// All filter dimensions are conjunctive:
/// a candidate MUST satisfy `asset AND amount AND lock`.
///
/// Defaults are unconstrained:
/// - `asset = none`
/// - `amount = none`
/// - `lock = none`
///
/// With all defaults, any unused wallet UTXO is eligible.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WalletSourceFilter {
    pub asset: AssetFilter,
    pub amount: AmountFilter,
    pub lock: LockFilter,
}

/// Source selector for one input prevout.
///
/// # Resolution Behavior
///
/// - `Wallet`:
///   candidates are filtered by [`WalletSourceFilter`], then selected by
///   a deterministic deficit-aware score (not first-match).
/// - `Provided`:
///   runtime fetches the prevout by outpoint from esplora and resolves
///   unblinding with [`InputBlinder`].
///
/// # Determinism
///
/// Wallet selection is deterministic for a fixed wallet snapshot and request,
/// but it is state-aware: declared input order can affect later selections
/// because earlier inputs update supply/deficit state.
///
/// # Failure Modes
///
/// - malformed `provided.outpoint` is rejected during request decoding
/// - no wallet candidate matching a `wallet` filter yields funding failure
/// - duplicate outpoint use within one request is rejected
/// - for `provided`, outpoint existence is checked at tx/vout fetch time;
///   there is no schema-level guarantee that the outpoint is currently unspent
///
/// # Security and UX Notes
///
/// - `provided.outpoint` is caller-controlled and MUST be treated as untrusted input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UTXOSource {
    Wallet { filter: WalletSourceFilter },
    Provided { outpoint: OutPoint },
}

/// Defaults to wallet-sourced input selection with an unconstrained filter.
impl Default for UTXOSource {
    fn default() -> Self {
        Self::Wallet {
            filter: WalletSourceFilter::default(),
        }
    }
}

/// Issuance mode for [`InputSchema::issuance`].
///
/// Serialized values are:
/// - `"new"`
/// - `"reissue"`
///
/// Variant compatibility with output asset variants:
/// - [`Self::New`] supports `new_issuance_asset` and `new_issuance_token`.
/// - [`Self::Reissue`] supports `re_issuance_asset`.
///
/// Mismatched output asset variants are rejected during runtime resolution with
/// `WalletAbiError::InvalidRequest`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputIssuanceKind {
    New,
    Reissue,
}

/// Issuance metadata attached to one transaction input.
///
/// This object is schema-level metadata only. Runtime consumes it in two places:
/// - input resolution, to populate issuance fields in the PSET input
/// - output resolution, to derive issuance-linked `AssetId` values
///
/// # Field semantics
///
/// - `kind`: issuance mode (`"new"` or `"reissue"`).
/// - `asset_amount_sat`: amount of issued/reissued asset units to mint.
/// - `token_amount_sat`: amount of reissuance token units to mint.
/// - `entropy`: 32-byte entropy payload whose meaning depends on `kind`.
///
/// # Entropy interpretation
///
/// - `kind = "new"`: `entropy` is interpreted as contract-hash entropy.
///   Runtime derives issuance entropy from `(selected_input_outpoint, contract_hash_entropy)`.
/// - `kind = "reissue"`: `entropy` is interpreted as already-derived asset entropy.
///   Runtime uses it directly.
///
/// # Security and UX notes
///
/// - Issuance-derived output assets are coupled to `input_index` in output asset variants.
///   `input_index` is positional in `params.inputs`; reordering inputs changes derived asset ids.
///
/// # Troubleshooting
///
/// Common request errors and likely causes:
/// - `"references missing input_index"`: output asset variant points to an out-of-range input.
/// - `"has no issuance metadata"`: output references issuance-derived asset, but input has no
///   `issuance`.
/// - `"new_issuance_* references non-new issuance"`: output variant requires `kind = "new"`.
/// - `"re_issuance_asset references non-reissue"`: output variant requires `kind = "reissue"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputIssuance {
    pub kind: InputIssuanceKind,
    pub asset_amount_sat: u64,
    pub token_amount_sat: u64,
    pub entropy: [u8; 32],
}

/// Source of taproot internal key material used by [`FinalizerSpec::Simf`].
///
/// # Purpose
///
/// `InternalKeySource` controls which x-only internal key is used in two runtime
/// paths:
/// - output lock resolution via [`FinalizerSpec::try_resolve_script_pubkey`]
/// - Simplicity finalization control-block construction in runtime finalization
///
/// # Runtime semantics
///
/// - `"bip0341"` uses fixed BIP-0341 example internal key bytes.
/// - `"external"` uses `key.pubkey` as the x-only source for witness/control
///   block operations.
/// - In [`FinalizerSpec::try_resolve_script_pubkey`], `"external"` MUST resolve
///   `source_simf` + `arguments`, derive the expected script from `key.pubkey`,
///   and reject mismatches against `key.address`.
///
/// # Security considerations
///
/// - Using `"external"` can imply that key-path spendability exists outside this
///   schema.
/// - A malformed `"external"` payload can contain internally inconsistent
///   `pubkey`/`address`/argument binding. Untrusted handles SHOULD be validated
///   with [`TaprootPubkeyGen::build_from_str`] before embedding.
/// - [`Self::get_x_only_pubkey`] is extraction-only and performs no consistency
///   checks against `address` or Simplicity program arguments.
///
/// # UX / integrator guidance
///
/// - Use `"bip0341"` for deterministic, portable defaults and compatibility with
///   existing BIP-0341-based templates.
/// - Use `"external"` when a different internal key is needed rather than `"bip0341"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InternalKeySource {
    /// Deterministic BIP-0341 example internal key bytes.
    ///
    /// This variant is the default.
    #[default]
    Bip0341,
    /// Externally supplied Taproot handle.
    ///
    /// Integrators SHOULD construct this value from a validated handle string via
    /// [`TaprootPubkeyGen::build_from_str`] rather than hand-building JSON fields.
    External { key: Box<TaprootPubkeyGen> },
}

impl InternalKeySource {
    /// Return the x-only key for witness/control-block operations.
    ///
    /// This method is a strict extraction rule:
    /// - `BIP0341` returns fixed BIP-0341 example key bytes
    /// - `External` returns x-only pubkey material from `key.pubkey`
    ///
    /// This method does **not** validate that `External.key.address` matches the
    /// extracted pubkey or the declared Simplicity program/arguments.
    pub fn get_x_only_pubkey(&self) -> XOnlyPublicKey {
        match self {
            Self::Bip0341 => bip_0341_example_internal_key(),
            Self::External { key } => key.get_x_only_pubkey(),
        }
    }
}

/// Fixed BIP-0341 example internal key bytes.
///
/// Provenance: <https://en.bitcoin.it/wiki/BIP_0341>.
///
/// Compatibility note:
/// the exact 32-byte value is part of the `wallet-abi-0.1` behavior contract and
/// MUST remain stable unless the ABI is versioned accordingly.
fn bip_0341_example_internal_key() -> XOnlyPublicKey {
    #[allow(clippy::unwrap_used)]
    XOnlyPublicKey::from_slice(&[
        0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a,
        0x5e, 0x07, 0x8a, 0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80,
        0x3a, 0xc0,
    ])
    .expect("bip-0341 key is valid")
}

/// Input finalization strategy attached to each [`InputSchema`].
///
/// # Wire format
///
/// This enum is externally tagged with `type` in `snake_case`.
///
/// # Runtime usage
///
/// - input resolution persists this payload in PSET proprietary `finalizer-spec`
/// - finalization reads it back to choose wallet or Simplicity path
/// - `LockVariant::Finalizer` output locking uses it to derive a script pubkey
///
/// # `simf` + internal-key nuance
///
/// - `internal_key = "bip0341"`:
///   script pubkey is derived from `(source_simf, arguments, network)` plus fixed
///   BIP-0341 key bytes.
/// - `internal_key = { "external": ... }`:
///   script pubkey is derived from `(source_simf, arguments, key.pubkey, network)`
///   and must match `external.key.address`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FinalizerSpec {
    /// Delegate input finalization to wallet signer/miniscript stack.
    ///
    /// Valid for input spending, but invalid for output lock derivation.
    #[default]
    Wallet,
    /// Finalize with an embedded Simplicity source, arguments and witness payload.
    Simf {
        /// UTF-8 Simplicity source code loaded and instantiated at runtime.
        source_simf: String,
        /// Taproot internal key source.
        ///
        /// Security note:
        /// callers SHOULD prefer validated [`TaprootPubkeyGen`] material when using
        /// the `External` branch.
        internal_key: InternalKeySource,
        /// UTF-8 JSON bytes of [`crate::wallet_abi::schema::values::SimfArguments`].
        ///
        /// Request JSON nuance:
        /// as `Vec<u8>`, this appears as an array of integers `[0..255]`.
        arguments: Vec<u8>,
        /// UTF-8 JSON bytes of [`crate::wallet_abi::schema::values::SimfWitness`].
        ///
        /// Request JSON nuance:
        /// as `Vec<u8>`, this appears as an array of integers `[0..255]`.
        witness: Vec<u8>,
    },
}

impl FinalizerSpec {
    /// Resolve script pubkey for `LockVariant::Finalizer`.
    ///
    /// Behavior contract:
    /// - `Wallet` returns `InvalidRequest` because no Simplicity lock is present.
    /// - `Simf + BIP0341` resolves arguments, loads `source_simf`, then derives a
    ///   Taproot script pubkey from program CMR + fixed BIP-0341 key + `network`.
    /// - `Simf + External` resolves arguments, loads `source_simf`, derives the
    ///   expected address from `key.pubkey` + `network`, and fails on
    ///   `key.address` mismatch.
    ///
    /// This is called from runtime output materialization.
    ///
    /// # Errors
    ///
    /// Returns `WalletAbiError` on invalid variant use, argument resolution errors, or
    /// Simplicity loading/derivation failures.
    pub fn try_resolve_script_pubkey(
        &self,
        pst: &PartiallySignedTransaction,
        network: lwk_common::Network,
    ) -> Result<Script, WalletAbiError> {
        let Self::Simf {
            source_simf,
            internal_key,
            arguments,
            ..
        } = self
        else {
            return Err(WalletAbiError::InvalidRequest(
                "trying to get runtime key from non-simplicity".to_string(),
            ));
        };

        let arguments = resolve_arguments(arguments, pst)?;
        let program = load_program(source_simf, arguments)?;
        let script = match internal_key {
            InternalKeySource::Bip0341 => create_p2tr_address(
                program.commit().cmr(),
                &bip_0341_example_internal_key(),
                network.address_params(),
            )
            .script_pubkey(),
            InternalKeySource::External { key } => {
                let expected_address = create_p2tr_address(
                    program.commit().cmr(),
                    &key.get_x_only_pubkey(),
                    network.address_params(),
                );
                if key.address != expected_address {
                    return Err(WalletAbiError::InvalidRequest(format!(
                        "external internal key mismatch: expected address {expected_address}, got {}",
                        key.address
                    )));
                }
                key.address.script_pubkey()
            }
        };

        Ok(script)
    }
}

impl FinalizerSpec {
    /// Serialize to UTF-8 JSON bytes for PSET proprietary metadata.
    ///
    /// # Errors
    ///
    /// Returns error if serialization fails.
    pub fn try_encode(&self) -> Result<Vec<u8>, WalletAbiError> {
        serde_json::to_vec(self).map_err(Into::into)
    }

    /// Decode from UTF-8 JSON bytes.
    ///
    /// Runtime uses this when reading `finalizer-spec` metadata from the PSET.
    ///
    /// # Errors
    ///
    /// Returns error if bytes are not a valid `FinalizerSpec` JSON payload.
    pub fn decode(bytes: &[u8]) -> Result<Self, WalletAbiError> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }
}

/// Unblinding strategy for the prevout selected by [`UTXOSource`].
///
/// # Purpose
///
/// Declare how runtime should obtain `TxOutSecrets` for this input's resolved
/// prevout so the transaction can be balanced and blinded safely.
///
/// # Behavior by Variant
///
/// - [`Self::Wallet`]:
///   runtime attempts wallet blinding-key derivation from the prevout script and
///   unblinds using wallet key material.
/// - [`Self::Provided`]:
///   runtime unblinds with the caller-supplied blinding `secret_key`.
/// - [`Self::Explicit`]:
///   prevout must already carry explicit asset/value fields; confidential values
///   are rejected.
///
/// # Security Considerations
///
/// - `secret_key` is a blinding key, not a spending/signing key.
/// - Never log or persist request payloads containing `provided.secret_key`.
/// - Runtime stores resolved input secrets in PSET proprietary metadata during
///   construction. Treat intermediate PSETs as sensitive material.
///
/// # UX Guidance
///
/// - Use [`Self::Explicit`] only when the referenced prevout is known explicit.
/// - Mismatched assumptions produce deterministic request errors, for example:
///   `"unable to unblind input ... with provided blinder"` or
///   `"marked input ... as explicit when the confidential was provided"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputBlinder {
    /// Use wallet-derived blinding key material.
    #[default]
    Wallet,
    /// Use caller-supplied input blinding key material.
    Provided { secret_key: SecretKey },
    /// Require explicit (non-confidential) prevout asset and value.
    Explicit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct InputSchema {
    /// User-provided identifier for the input
    pub id: String,
    /// Declares how runtime discovers the prevout for this input.
    ///
    /// This field controls source resolution only. It does not by itself guarantee
    /// spend authorization, witness satisfiability, or finalization success.
    pub utxo_source: UTXOSource,
    /// Declares how the resolved prevout is unblinded.
    ///
    /// Practical behavior matrix (`wallet-abi-0.1`):
    ///
    /// | `utxo_source` | `blinder` | Runtime behavior |
    /// | --- | --- | --- |
    /// | `wallet` | any variant | Uses wallet snapshot unblinded material for the selected wallet UTXO. |
    /// | `provided` | `wallet` | Derives wallet blinding key from prevout script and attempts unblinding. |
    /// | `provided` | `provided` | Attempts unblinding with caller-provided `secret_key`. |
    /// | `provided` | `explicit` | Requires explicit prevout asset/value (confidential prevouts fail). |
    ///
    /// Recommendation:
    /// callers should still set semantically matching source+blinder pairs to
    /// reduce confusion and future migration risk.
    ///
    /// Security reminders:
    /// - `provided.secret_key` should never be logged or persisted.
    /// - Intermediate PSETs contain resolved input secrets in proprietary metadata.
    ///
    /// UX reminders:
    /// - [`InputBlinder::Explicit`] should be used only when explicit prevouts are expected.
    /// - Misclassification leads to deterministic runtime errors.
    pub blinder: InputBlinder,
    /// Bitcoin transaction input sequence number.
    ///
    /// The sequence field is used for:
    /// - Indicating whether absolute lock-time (specified in `lock_time` field of [`RuntimeParams`])
    ///   is enabled.
    /// - Indicating and encoding [BIP-68] relative lock-times.
    /// - Indicating whether a transaction opts-in to [BIP-125] replace-by-fee.
    pub sequence: Sequence,
    /// Optional issuance metadata attached to this input.
    ///
    /// Required when an output asset variant references this input via
    /// `new_issuance_asset`, `new_issuance_token`, or `re_issuance_asset`.
    ///
    /// References are positional (`input_index` in `params.outputs[*].asset`), so changing input
    /// order changes which issuance metadata and outpoint are used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuance: Option<InputIssuance>,
    /// Input finalization strategy attached to each `InputSchema`.
    pub finalizer: FinalizerSpec,
}

impl InputSchema {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Default::default()
        }
    }

    /// Attach issuance metadata to this input and return the updated schema.
    ///
    /// # Purpose
    ///
    /// This is a builder-style convenience helper for request construction:
    /// it sets [`InputSchema::issuance`] and returns `self` for chaining.
    ///
    /// # Semantics
    ///
    /// - The method only performs assignment:
    ///   `self.issuance = Some(issuance)`.
    /// - If called multiple times, the latest call overwrites the previous
    ///   issuance metadata.
    ///
    /// # Positional Coupling
    ///
    /// Issuance-derived output assets (`new_issuance_asset`,
    /// `new_issuance_token`, `re_issuance_asset`) reference inputs by
    /// positional `input_index` in `params.inputs`.
    ///
    /// Reordering inputs therefore changes which issuance metadata and outpoint
    /// drive derived asset ids.
    ///
    /// # Validation
    ///
    /// This method performs no cross-field or cross-input validation.
    /// Compatibility checks (for example output asset variant vs issuance kind)
    /// are enforced later during runtime resolution.
    ///
    /// # Security and UX Notes
    ///
    /// - Issuance metadata materially affects asset-id derivation and supply.
    ///   Treat it as high-impact request input.
    /// - Prefer stable declared-input ordering once outputs reference
    ///   `input_index` values, to avoid surprising derived-asset changes.
    pub const fn with_issuance(mut self, issuance: InputIssuance) -> Self {
        self.issuance = Some(issuance);
        self
    }
}

/// Output lock selector for [`OutputSchema::lock`].
///
/// # Purpose and scope
///
/// `LockVariant` declares how runtime determines the script pubkey for one output.
/// It does not by itself prove spendability or policy correctness of the target lock.
///
/// # Runtime semantics
///
/// - `Script` is used directly as the output script pubkey.
/// - `Finalizer` delegates script derivation to
///   [`FinalizerSpec::try_resolve_script_pubkey`].
///
/// # Nuance notes
///
/// - `FinalizerSpec::Simf + InternalKeySource::External` validates that
///   `key.address` matches script derivation from
///   `(source_simf, arguments, key.pubkey, network)`.
/// - A mismatch is rejected with `WalletAbiError::InvalidRequest`.
///
/// # Security notes
///
/// - `Script` carries caller-controlled raw script bytes; runtime does not validate
///   ownership intent or spend policy.
/// - `External` taproot handle usage crosses a trust boundary and should be
///   validated upstream before embedding into requests.
///
/// # UX notes
///
/// - Prefer `script` when directly targeting a known address/script.
/// - Prefer `finalizer` when lock derivation is intentionally contract-driven.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LockVariant {
    Script { script: Script },
    Finalizer { finalizer: Box<FinalizerSpec> },
}

impl Default for LockVariant {
    fn default() -> Self {
        Self::Finalizer {
            finalizer: Box::new(FinalizerSpec::default()),
        }
    }
}

/// Asset selector for an output.
///
/// # Positional semantics
///
/// Issuance-linked variants resolve deterministically from the referenced input index and its
/// issuance metadata. `input_index` is positional in `RuntimeParams.inputs`.
/// Reordering inputs changes which outpoint/issuance metadata is referenced.
///
/// # Runtime checks
///
/// Runtime validates index bounds and issuance-kind compatibility:
/// - `new_issuance_asset` and `new_issuance_token` require
///   `params.inputs[input_index].issuance.kind == "new"`.
/// - `re_issuance_asset` requires
///   `params.inputs[input_index].issuance.kind == "reissue"`.
///
/// Violations fail deterministically with `WalletAbiError::InvalidRequest`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssetVariant {
    /// Use an explicit caller-provided asset id.
    AssetId { asset_id: AssetId },
    /// Use the issued asset id derived from input issuance metadata.
    ///
    /// Requires `params.inputs[input_index].issuance.kind == "new"`.
    NewIssuanceAsset { input_index: u32 },
    /// Use the reissuance token asset id derived from input issuance metadata.
    ///
    /// Requires `params.inputs[input_index].issuance.kind == "new"`.
    NewIssuanceToken { input_index: u32 },
    /// Use the reissued asset id derived from input issuance metadata.
    ///
    /// Requires `params.inputs[input_index].issuance.kind == "reissue"`.
    ReIssuanceAsset { input_index: u32 },
}

/// Output blinding policy selector.
///
/// # Runtime mapping
///
/// During output materialization:
/// - `Wallet` => output `blinding_key = signer_receive_address().blinding_pubkey`.
/// - `Provided` => output `blinding_key = provided.pubkey`.
/// - `Explicit` => output `blinding_key = None`.
///
/// Runtime does not cross-check `lock.script` and `provided.pubkey`; they are treated
/// as independent caller-controlled fields.
///
/// # Security and UX notes
///
/// - `Explicit` disables output amount/asset confidentiality for that output.
/// - `Wallet` is usually correct for wallet-owned receive/change outputs.
/// - Using `Wallet` for non-wallet recipients can result in Simplicity spending failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlinderVariant {
    #[default]
    Wallet,
    Provided {
        pubkey: PublicKey,
    },
    Explicit,
}

/// One requested output entry in `RuntimeParams.outputs`.
///
/// This struct declares requested outputs only. Runtime may additionally:
/// - normalize or append the fee output (`id == "fee"` semantics),
/// - append change outputs for residual balances.
///
/// # Field semantics
///
/// - `id`: caller label used for diagnostics and special fee placeholder handling.
///   - `id == "fee"` marks a fee placeholder candidate.
///   - duplicate `"fee"` ids are rejected.
/// - `amount_sat`: requested amount for this output.
///   - if `id == "fee"`, runtime overwrites this value with the resolved fee target.
/// - `lock`: locking rule (`script` or finalizer-derived script).
/// - `asset`: explicit or issuance-derived asset selector.
/// - `blinder`: output blinding policy.
///
/// # Coupling and caveats
///
/// Runtime does not enforce semantic coupling between `lock` and `blinder` payloads.
/// Callers should ensure script ownership and blinding-key ownership are coherent for
/// the intended recipient flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputSchema {
    /// Output identifier used for diagnostics and fee placeholder recognition.
    pub id: String,
    /// Requested output amount in satoshis.
    ///
    /// For `id == "fee"`, this field is treated as placeholder and replaced by runtime.
    pub amount_sat: u64,
    /// Locking selector for this output.
    pub lock: LockVariant,
    /// Asset selector for this output.
    pub asset: AssetVariant,
    /// Blinding policy selector for this output.
    pub blinder: BlinderVariant,
}
