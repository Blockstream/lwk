use std::str::FromStr;
use std::sync::{Arc, Mutex};

use elements::bitcoin::bip32::DerivationPath;
use lwk_simplicity_options::error::PsetBuilderError;
use lwk_simplicity_options::pset_builder_dp::{
    self, SimplicityInputSpec, SimplicityOutputSpec, SimplicityPsetBuilder as CoreBuilder,
    SimplicityPsetInput, PLACEHOLDER_FEE,
};
use lwk_simplicity_options::simplicityhl;
use lwk_simplicity_options::utils::{convert_values_to_map, parse_genesis_hash};

use crate::blockdata::tx_out::TxOut;
use crate::types::{AssetId, Hex, XOnlyPublicKey};
use crate::{
    Address, LwkError, Network, OutPoint, Pset, Signer, SimplicityLogLevel, SimplicityProgram,
    SimplicityWitnessValues, Transaction, TxOutSecrets,
};

/// Trait for building Simplicity witness values from a signature.
#[uniffi::export(with_foreign)]
pub trait WitnessBuilder: Send + Sync {
    /// Build witness values given a signature.
    fn build(&self, signature: Hex) -> Result<Arc<SimplicityWitnessValues>, LwkError>;
}

/// FFI-specific data for each input (not needed by core builder).
struct InputFfiData {
    derivation_path: Option<String>,
    witness_builder: Option<Arc<dyn WitnessBuilder>>,
    pre_built_witness: Option<Arc<SimplicityWitnessValues>>,
}

struct SimplicityPsetBuilderInner {
    /// Core builder containing all business logic.
    core: CoreBuilder,
    /// Signer for key derivation and signing.
    signer: Option<Arc<Signer>>,
    /// FFI-specific data per input (parallel to core.inputs).
    input_ffi_data: Vec<InputFfiData>,
    /// Log level for Simplicity program execution.
    log_level: simplicityhl::tracker::TrackerLogLevel,
}

/// A PSET-based transaction builder for Simplicity programs.
#[derive(uniffi::Object)]
pub struct SimplicityPsetBuilder {
    inner: Mutex<Option<SimplicityPsetBuilderInner>>,
}

fn builder_finished() -> LwkError {
    LwkError::ObjectConsumed
}

fn xonly_to_simplicity(
    key: &XOnlyPublicKey,
) -> Result<simplicityhl::simplicity::bitcoin::XOnlyPublicKey, LwkError> {
    simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(&key.serialize()).map_err(|e| {
        LwkError::Generic {
            msg: format!("Invalid x-only public key: {e}"),
        }
    })
}

fn pset_error_to_lwk(e: PsetBuilderError) -> LwkError {
    LwkError::Generic { msg: e.to_string() }
}

#[uniffi::export]
impl SimplicityPsetBuilder {
    /// Create a new SimplicityPsetBuilder.
    #[uniffi::constructor]
    pub fn new(network: &Network, genesis_hash: Hex) -> Result<Arc<Self>, LwkError> {
        let elements_network: lwk_wollet::ElementsNetwork = network.into();
        let common_network: lwk_common::Network = elements_network.into();
        let policy_asset = elements_network.policy_asset();
        let genesis =
            parse_genesis_hash(genesis_hash.as_ref()).map_err(|msg| LwkError::Generic {
                msg: msg.to_string(),
            })?;

        let core = CoreBuilder::new(common_network, policy_asset, genesis);

        Ok(Arc::new(Self {
            inner: Mutex::new(Some(SimplicityPsetBuilderInner {
                core,
                signer: None,
                input_ffi_data: Vec::new(),
                log_level: simplicityhl::tracker::TrackerLogLevel::None,
            })),
        }))
    }

    /// Set the signer for signing inputs.
    pub fn set_signer(&self, signer: Arc<Signer>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;
        inner.signer = Some(signer);
        Ok(())
    }

    /// Set the fee rate in sats/kvb.
    pub fn fee_rate(&self, rate: f32) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;
        inner.core.set_fee_rate(rate);
        Ok(())
    }

    /// Set the log level for Simplicity program execution.
    pub fn set_log_level(&self, log_level: SimplicityLogLevel) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;
        inner.log_level = log_level.into();
        Ok(())
    }

    /// Add a Simplicity input that will be signed during `finish()`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_simplicity_input_signed(
        &self,
        program: Arc<SimplicityProgram>,
        internal_key: &XOnlyPublicKey,
        outpoint: &OutPoint,
        utxo: Arc<TxOut>,
        utxo_secrets: Arc<TxOutSecrets>,
        derivation_path: String,
        witness_builder: Arc<dyn WitnessBuilder>,
    ) -> Result<(), LwkError> {
        DerivationPath::from_str(&derivation_path).map_err(|e| LwkError::Generic {
            msg: format!("Invalid derivation path: {e}"),
        })?;

        let x_only_key = xonly_to_simplicity(internal_key)?;

        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;

        let input_index = inner.core.n_inputs();

        // Add to core builder
        inner.core.add_input(SimplicityPsetInput {
            spec: SimplicityInputSpec {
                program: program.inner.clone(),
                internal_key: x_only_key,
                input_index,
            },
            outpoint: outpoint.into(),
            utxo: utxo.as_ref().into(),
        });

        // Store FFI-specific data
        inner.input_ffi_data.push(InputFfiData {
            derivation_path: Some(derivation_path),
            witness_builder: Some(witness_builder),
            pre_built_witness: None,
        });

        // Suppress unused warning - secrets may be needed for blinding in future
        let _ = utxo_secrets;

        Ok(())
    }

    /// Add a Simplicity input with a pre-built witness (no signing).
    #[allow(clippy::too_many_arguments)]
    pub fn add_simplicity_input(
        &self,
        program: Arc<SimplicityProgram>,
        internal_key: &XOnlyPublicKey,
        outpoint: &OutPoint,
        utxo: Arc<TxOut>,
        utxo_secrets: Arc<TxOutSecrets>,
        witness: Arc<SimplicityWitnessValues>,
    ) -> Result<(), LwkError> {
        let x_only_key = xonly_to_simplicity(internal_key)?;

        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;

        let input_index = inner.core.n_inputs();

        // Add to core builder
        inner.core.add_input(SimplicityPsetInput {
            spec: SimplicityInputSpec {
                program: program.inner.clone(),
                internal_key: x_only_key,
                input_index,
            },
            outpoint: outpoint.into(),
            utxo: utxo.as_ref().into(),
        });

        // Store FFI-specific data
        inner.input_ffi_data.push(InputFfiData {
            derivation_path: None,
            witness_builder: None,
            pre_built_witness: Some(witness),
        });

        // Suppress unused warning - secrets may be needed for blinding in future
        let _ = utxo_secrets;

        Ok(())
    }

    /// Add a recipient receiving L-BTC.
    pub fn add_lbtc_recipient(&self, address: &Address, satoshi: u64) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;
        inner.core.add_output(SimplicityOutputSpec::LbtcRecipient {
            address: address.into(),
            satoshi,
        });
        Ok(())
    }

    /// Add a recipient receiving a specific asset.
    pub fn add_recipient(
        &self,
        address: &Address,
        satoshi: u64,
        asset: &AssetId,
    ) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;
        inner.core.add_output(SimplicityOutputSpec::AssetRecipient {
            address: address.into(),
            satoshi,
            asset: (*asset).into(),
        });
        Ok(())
    }

    /// Set the address to drain excess L-BTC to (change address).
    pub fn drain_lbtc_to(&self, address: &Address) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_finished)?;
        inner.core.add_output(SimplicityOutputSpec::DrainLbtcTo {
            address: address.into(),
        });
        Ok(())
    }

    /// Build an unsigned PSET with placeholder fee.
    pub fn build_unsigned(&self) -> Result<Arc<Pset>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(builder_finished)?;

        let pset = inner
            .core
            .build_pset(PLACEHOLDER_FEE)
            .map_err(pset_error_to_lwk)?;
        Ok(Arc::new(pset.into()))
    }

    /// Build, sign, and finalize the transaction, returning a PSET.
    pub fn finish(&self) -> Result<Arc<Pset>, LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;

        let signer = inner.signer.clone();
        let input_ffi_data = inner.input_ffi_data;
        let log_level = inner.log_level;

        // Create signer callback that adapts FFI types to core
        let signer_callback = |input_idx: usize,
                               spec: &SimplicityInputSpec,
                               sighash: &elements::secp256k1_zkp::Message|
         -> Result<simplicityhl::WitnessValues, PsetBuilderError> {
            let ffi_data = input_ffi_data
                .get(input_idx)
                .ok_or(PsetBuilderError::MissingWitness(input_idx))?;

            let witness_inner = if let Some(derivation_path) = &ffi_data.derivation_path {
                // Signed input: derive key, sign, build witness
                let signer_ref = signer.as_ref().ok_or_else(|| {
                    PsetBuilderError::Signing("Signer not set but signed input added".to_string())
                })?;

                let path = DerivationPath::from_str(derivation_path).map_err(|e| {
                    PsetBuilderError::Signing(format!("Invalid derivation path: {e}"))
                })?;

                let derived_xprv = signer_ref.inner.derive_xprv(&path).map_err(|e| {
                    PsetBuilderError::Signing(format!("Key derivation failed: {e}"))
                })?;

                let keypair = elements::bitcoin::secp256k1::Keypair::from_secret_key(
                    elements::bitcoin::secp256k1::SECP256K1,
                    &derived_xprv.private_key,
                );
                let x_only_pubkey = keypair.x_only_public_key().0;

                // Verify derived key matches internal key
                let derived_key_bytes = x_only_pubkey.serialize();
                if spec.internal_key.serialize() != derived_key_bytes {
                    return Err(PsetBuilderError::Signing(format!(
                        "internal_key does not match derived key from derivation path: expected {}, got {}",
                        Hex::from(derived_key_bytes.to_vec()),
                        Hex::from(spec.internal_key.serialize().to_vec())
                    )));
                }

                let signature = keypair.sign_schnorr(*sighash);
                let sig_hex = Hex::from(signature.serialize().to_vec());

                let witness_builder = ffi_data.witness_builder.as_ref().ok_or_else(|| {
                    PsetBuilderError::Signing("No witness builder for signed input".to_string())
                })?;

                let witness = witness_builder.build(sig_hex).map_err(|e| {
                    PsetBuilderError::Signing(format!("Witness builder failed: {e}"))
                })?;

                simplicityhl::WitnessValues::from(convert_values_to_map(witness.inner_map()))
            } else {
                // Pre-built witness input
                let witness = ffi_data
                    .pre_built_witness
                    .as_ref()
                    .ok_or(PsetBuilderError::MissingWitness(input_idx))?;

                simplicityhl::WitnessValues::from(convert_values_to_map(witness.inner_map()))
            };

            Ok(witness_inner)
        };

        let signed = inner
            .core
            .finish(signer_callback, log_level)
            .map_err(pset_error_to_lwk)?;
        Ok(Arc::new(signed.into()))
    }

    /// Build, sign, and finalize the transaction, returning the extracted transaction.
    pub fn finish_tx(&self) -> Result<Arc<Transaction>, LwkError> {
        let pset = self.finish()?;
        pset.extract_tx()
    }
}

/// Finalize a PSET input with a Simplicity witness.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn simplicity_finalize_pset_input(
    pset: &Pset,
    program: &SimplicityProgram,
    internal_key: &XOnlyPublicKey,
    input_index: u32,
    utxos: Vec<Arc<TxOut>>,
    witness_values: &SimplicityWitnessValues,
    network: &Network,
    genesis_hash: Hex,
    log_level: SimplicityLogLevel,
) -> Result<Arc<Pset>, LwkError> {
    let x_only_key = xonly_to_simplicity(internal_key)?;
    let genesis = parse_genesis_hash(genesis_hash.as_ref()).map_err(|msg| LwkError::Generic {
        msg: msg.to_string(),
    })?;

    let elements_network: lwk_wollet::ElementsNetwork = network.into();
    let common_network: lwk_common::Network = elements_network.into();

    let spec = SimplicityInputSpec {
        program: program.inner.clone(),
        internal_key: x_only_key,
        input_index: input_index as usize,
    };

    let utxos_inner: Vec<elements::TxOut> = utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect();

    let witness_inner =
        simplicityhl::WitnessValues::from(convert_values_to_map(witness_values.inner_map()));

    let result = pset_builder_dp::finalize_simplicity_input(
        pset.as_ref().clone(),
        &spec,
        &utxos_inner,
        witness_inner,
        common_network,
        genesis,
        log_level.into(),
    )
    .map_err(|e| LwkError::Generic { msg: e.to_string() })?;

    Ok(Arc::new(result.into()))
}

/// Get the sighash for signing a Simplicity PSET input.
#[uniffi::export]
pub fn simplicity_get_pset_sighash(
    pset: &Pset,
    program: &SimplicityProgram,
    internal_key: &XOnlyPublicKey,
    input_index: u32,
    utxos: Vec<Arc<TxOut>>,
    network: &Network,
    genesis_hash: Hex,
) -> Result<Hex, LwkError> {
    let x_only_key = xonly_to_simplicity(internal_key)?;
    let genesis = parse_genesis_hash(genesis_hash.as_ref()).map_err(|msg| LwkError::Generic {
        msg: msg.to_string(),
    })?;

    let elements_network: lwk_wollet::ElementsNetwork = network.into();
    let common_network: lwk_common::Network = elements_network.into();

    let spec = SimplicityInputSpec {
        program: program.inner.clone(),
        internal_key: x_only_key,
        input_index: input_index as usize,
    };

    let utxos_inner: Vec<elements::TxOut> = utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect();

    let message = pset_builder_dp::get_simplicity_sighash(
        pset.as_ref(),
        &spec,
        &utxos_inner,
        common_network,
        genesis,
    )
    .map_err(|e| LwkError::Generic { msg: e.to_string() })?;

    Ok(Hex::from(message.as_ref().to_vec()))
}
