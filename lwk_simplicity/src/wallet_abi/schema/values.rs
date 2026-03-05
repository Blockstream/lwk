//! SIMF argument and witness payload codecs used by `wallet-abi-0.1`.

use crate::error::WalletAbiError;
use crate::simplicityhl::num::U256;
use crate::simplicityhl::parse::ParseFromStr;
use crate::simplicityhl::simplicity::jet::elements::ElementsEnv;
use crate::simplicityhl::str::WitnessName;
use crate::simplicityhl::value::{UIntValue, ValueConstructible};
use crate::simplicityhl::{Arguments, Value, WitnessValues};

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use lwk_wollet::elements::pset::{Input, PartiallySignedTransaction};
use lwk_wollet::elements::secp256k1_zkp::ZERO_TWEAK;
use lwk_wollet::elements::Transaction;
use lwk_wollet::hashes::Hash;
use lwk_wollet::secp256k1::{Keypair, Message, XOnlyPublicKey};

/// Runtime-resolved Simplicity argument sources.
///
/// Serialization uses `snake_case` variant names.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSimfValue {
    /// Resolve witness value from the issuance asset id of one **new issuance** input.
    ///
    /// `input_index` is the zero-based index in `PartiallySignedTransaction::inputs()`.
    /// The referenced input must contain issuance metadata and be a new issuance.
    NewIssuanceAsset { input_index: u32 },
    /// Resolve witness value from the reissuance token id of one **new issuance** input.
    ///
    /// `input_index` is the zero-based index in `PartiallySignedTransaction::inputs()`.
    /// The referenced input must contain issuance metadata and be a new issuance.
    NewIssuanceToken { input_index: u32 },
}

/// Simplicity argument payload used by `FinalizerSpec::Simf`.
///
/// Values are split into:
/// - static `resolved` arguments supplied directly by the caller;
/// - `runtime_arguments` that are derived from the concrete PSET at finalization time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimfArguments {
    /// Caller-supplied static witness values.
    ///
    /// Keys are `simplicityhl::str::WitnessName` values and must already be type-correct
    /// for the target Simplicity program.
    pub resolved: Arguments,
    /// Runtime-derived witness values keyed by witness name.
    ///
    /// Keys are parsed using `WitnessName::parse_from_str` during resolution. Any key that
    /// collides with `resolved` is rejected.
    pub runtime_arguments: HashMap<String, RuntimeSimfValue>,
}

impl SimfArguments {
    /// Build payload with only static arguments.
    pub fn new(static_arguments: Arguments) -> Self {
        Self {
            resolved: static_arguments,
            runtime_arguments: HashMap::new(),
        }
    }

    /// Add or replace one runtime-resolved argument by witness name.
    pub fn append_runtime_simf_value(&mut self, name: &str, runtime_simf_value: RuntimeSimfValue) {
        self.runtime_arguments
            .insert(name.to_string(), runtime_simf_value);
    }
}

/// Serialize `SimfArguments` into UTF-8 JSON bytes.
///
/// Wire shape:
/// - `resolved`: map-like JSON encoding of `simplicityhl::Arguments`
/// - `runtime_arguments`: object keyed by witness name strings
///
/// # Errors
///
/// Returns an error when arguments serialization failed
pub fn serialize_arguments(arguments: &SimfArguments) -> Result<Vec<u8>, WalletAbiError> {
    Ok(serde_json::to_vec(arguments)?)
}

/// Deserialize and resolve final Simplicity arguments from JSON bytes.
///
/// Resolution flow:
/// 1. Decode `SimfArguments`.
/// 2. Insert static `resolved` entries.
/// 3. Parse each runtime map key as `WitnessName`.
/// 4. Reject any runtime/static witness-name collisions.
/// 5. Resolve runtime entries from referenced PSET inputs (`input_index`).
///
/// Runtime issuance values are encoded as `u256` values using the raw 32-byte payload from
/// `AssetId::into_inner().0`.
///
/// # Errors
///
/// - invalid JSON or unexpected shape returns `Serde`.
/// - invalid witness names return `InvalidRequest`.
/// - runtime/static witness-name collisions returns `InvalidRequest`.
pub fn resolve_arguments(
    bytes: &[u8],
    pst: &PartiallySignedTransaction,
) -> Result<Arguments, WalletAbiError> {
    let simf_arguments: SimfArguments = serde_json::from_slice(bytes)?;

    let mut final_arguments: HashMap<WitnessName, Value> = HashMap::<WitnessName, Value>::new();

    for static_arg in simf_arguments.resolved.iter() {
        final_arguments.insert(static_arg.0.clone(), static_arg.1.clone());
    }

    for (name, value) in simf_arguments.runtime_arguments {
        let witness_name = parse_witness_name(&name, "runtime argument map")?;
        if final_arguments.contains_key(&witness_name) {
            return Err(WalletAbiError::InvalidRequest(format!(
                "runtime Simplicity argument '{name}' collides with static resolved argument '{witness_name}'"
            )));
        }

        match value {
            RuntimeSimfValue::NewIssuanceAsset { input_index } => {
                let input =
                    resolve_new_issuance_input(pst, &name, input_index, "new_issuance_asset")?;
                let (asset, _) = input.issuance_ids();

                final_arguments.insert(
                    witness_name,
                    Value::from(UIntValue::U256(U256::from_byte_array(asset.into_inner().0))),
                );
            }
            RuntimeSimfValue::NewIssuanceToken { input_index } => {
                let input =
                    resolve_new_issuance_input(pst, &name, input_index, "new_issuance_token")?;
                let (_, token) = input.issuance_ids();

                final_arguments.insert(
                    witness_name,
                    Value::from(UIntValue::U256(U256::from_byte_array(token.into_inner().0))),
                );
            }
        }
    }

    Ok(Arguments::from(final_arguments))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSimfWitness {
    /// Inject a Schnorr signature over `env.sighash_all` at runtime.
    ///
    /// Semantics:
    /// - `name` must be a valid Simplicity witness identifier.
    /// - `public_key` must equal the runtime signer x-only public key.
    /// - signature bytes are produced with the runtime signer keypair over
    ///   `ElementsEnv::c_tx_env().sighash_all()`.
    SigHashAll {
        name: String,
        public_key: XOnlyPublicKey,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimfWitness {
    /// Statically resolved witness values serialized into the payload.
    pub resolved: WitnessValues,
    /// Runtime witness directives that are resolved during finalization.
    ///
    /// Resolution flow in [`resolve_witness`]:
    /// 1. Decode `SimfWitness`.
    /// 2. Insert static `resolved` entries.
    /// 3. Parse each runtime witness entry name as `WitnessName`.
    /// 4. Reject any runtime/static witness-name collisions.
    /// 5. Resolve each runtime directive in `runtime_arguments` in order.
    ///
    /// This is current behavior and intentionally preserved for
    /// `wallet-abi-0.1` compatibility.
    pub runtime_arguments: Vec<RuntimeSimfWitness>,
}

/// Serialize a [`SimfWitness`] into UTF-8 JSON bytes.
///
/// The resulting bytes are carried by `FinalizerSpec::Simf.witness` and later
/// decoded by [`resolve_witness`].
///
/// # Errors
///
/// Returns an error when arguments serialization failed
pub fn serialize_witness(witness: &SimfWitness) -> Result<Vec<u8>, WalletAbiError> {
    Ok(serde_json::to_vec(witness)?)
}

/// Deserialize and resolve Simplicity witness values from payload bytes.
///
/// Resolution flow in [`resolve_witness`]:
/// 1. Decode `SimfWitness`.
/// 2. Insert static `resolved` entries.
/// 3. Parse each runtime witness entry name as `WitnessName`.
/// 4. Reject any runtime/static witness-name collisions.
/// 5. Resolve each runtime directive in `runtime_arguments` in order.
///
/// # Errors
///
/// - invalid JSON or unexpected shape returns `Serde`.
/// - invalid witness names return `InvalidRequest`.
/// - `sig_hash_all` signer key mismatch returns `InvalidRequest`.
/// - runtime/static witness-name collisions returns `InvalidRequest`.
pub fn resolve_witness(
    bytes: &[u8],
    contract_signer: &Keypair,
    env: &ElementsEnv<Arc<Transaction>>,
) -> Result<WitnessValues, WalletAbiError> {
    let simf_arguments: SimfWitness = serde_json::from_slice(bytes)?;

    let mut final_witness: HashMap<WitnessName, Value> = HashMap::<WitnessName, Value>::new();

    for static_arg in simf_arguments.resolved.iter() {
        final_witness.insert(static_arg.0.clone(), static_arg.1.clone());
    }

    let sighash_all = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

    for value in simf_arguments.runtime_arguments {
        match value {
            RuntimeSimfWitness::SigHashAll { name, public_key } => {
                let signer_public_key = contract_signer.x_only_public_key().0;
                if signer_public_key != public_key {
                    return Err(WalletAbiError::InvalidRequest(format!(
                        "sighash_all witness '{name}' public key mismatch: expected {public_key}, runtime signer is {signer_public_key}"
                    )));
                }
                let witness_name = parse_witness_name(&name, "runtime witness list")?;

                if final_witness.contains_key(&witness_name) {
                    return Err(WalletAbiError::InvalidRequest(format!(
                        "runtime Simplicity witness '{name}' collides with static resolved witness '{witness_name}'"
                    )));
                }

                final_witness.insert(
                    witness_name,
                    Value::byte_array(contract_signer.sign_schnorr(sighash_all).serialize()),
                );
            }
        }
    }

    Ok(WitnessValues::from(final_witness))
}

fn parse_witness_name(name: &str, source: &str) -> Result<WitnessName, WalletAbiError> {
    WitnessName::parse_from_str(name).map_err(|error| {
        WalletAbiError::InvalidRequest(format!(
            "invalid Simplicity witness name '{name}' in {source}: {error}"
        ))
    })
}

fn resolve_new_issuance_input<'a>(
    pst: &'a PartiallySignedTransaction,
    name: &str,
    input_index: u32,
    variant: &str,
) -> Result<&'a Input, WalletAbiError> {
    let idx = usize::try_from(input_index).map_err(|_| {
        WalletAbiError::InvalidRequest(format!(
            "runtime Simplicity argument '{name}' input_index overflow: {input_index}"
        ))
    })?;

    let input = pst.inputs().get(idx).ok_or_else(|| {
        WalletAbiError::InvalidRequest(format!(
            "runtime Simplicity argument '{name}' references missing input_index {input_index} (pset inputs: {})",
            pst.inputs().len()
        ))
    })?;

    if !input.has_issuance() {
        return Err(WalletAbiError::InvalidRequest(format!(
            "runtime Simplicity argument '{name}' ({variant}) references input_index {input_index} without issuance metadata"
        )));
    }

    let is_new_issuance = input.issuance_blinding_nonce.unwrap_or(ZERO_TWEAK) == ZERO_TWEAK;
    if !is_new_issuance {
        return Err(WalletAbiError::InvalidRequest(format!(
            "runtime Simplicity argument '{name}' ({variant}) requires new issuance input_index {input_index}, but referenced input is reissuance"
        )));
    }

    Ok(input)
}
