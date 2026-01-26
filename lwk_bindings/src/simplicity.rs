use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::bip32::DerivationPath;
use lwk_simplicity_options::runner;
use lwk_simplicity_options::scripts;
use lwk_simplicity_options::signer;
use lwk_simplicity_options::simplicityhl;
use lwk_simplicity_options::utils::{
    convert_values_to_map, network_to_address_params, parse_genesis_hash, validate_bytes_length,
    SimplicityValue,
};

use crate::blockdata::tx_out::TxOut;
use crate::types::{Hex, XOnlyPublicKey};
use crate::{Address, LwkError, Network, Transaction};

/// Log level for Simplicity program execution tracing.
#[derive(uniffi::Enum, Clone, Copy, Debug, Default)]
pub enum SimplicityLogLevel {
    /// No output during execution.
    #[default]
    None,
    /// Print debug information.
    Debug,
    /// Print debug and warning information.
    Warning,
    /// Print debug, warning, and jet execution trace.
    Trace,
}

impl From<SimplicityLogLevel> for simplicityhl::tracker::TrackerLogLevel {
    fn from(level: SimplicityLogLevel) -> Self {
        match level {
            SimplicityLogLevel::None => simplicityhl::tracker::TrackerLogLevel::None,
            SimplicityLogLevel::Debug => simplicityhl::tracker::TrackerLogLevel::Debug,
            SimplicityLogLevel::Warning => simplicityhl::tracker::TrackerLogLevel::Warning,
            SimplicityLogLevel::Trace => simplicityhl::tracker::TrackerLogLevel::Trace,
        }
    }
}

/// Builder for Simplicity program arguments.
///
/// Arguments are named values that are substituted into the program source
/// during compilation (e.g., public keys, thresholds).
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityArguments {
    inner: HashMap<String, SimplicityValue>,
}

/// Macro to implement add_number and add_bytes for value builder types.
macro_rules! impl_value_builder {
    ($type:ty) => {
        #[uniffi::export]
        impl $type {
            /// Create a new empty builder.
            #[uniffi::constructor]
            pub fn new() -> Arc<Self> {
                Arc::new(Self::default())
            }

            /// Add a numeric value (handles u8, u16, u32, u64).
            pub fn add_number(&self, name: String, value: u64) -> Arc<Self> {
                let mut new = self.clone();
                new.inner.insert(name, SimplicityValue::Number(value));
                Arc::new(new)
            }

            /// Add a byte array value from hex string (32 or 64 bytes).
            pub fn add_bytes(&self, name: String, value: Hex) -> Result<Arc<Self>, LwkError> {
                let bytes = value.as_ref().to_vec();
                if let Some(msg) = validate_bytes_length(bytes.len()) {
                    return Err(LwkError::Generic { msg });
                }
                let mut new = self.clone();
                new.inner.insert(name, SimplicityValue::Bytes(bytes));
                Ok(Arc::new(new))
            }
        }
    };
}

impl_value_builder!(SimplicityArguments);

impl SimplicityArguments {
    fn to_inner(&self) -> simplicityhl::Arguments {
        simplicityhl::Arguments::from(convert_values_to_map(&self.inner))
    }
}

/// Builder for Simplicity witness values.
///
/// Witness values are runtime inputs provided when finalizing a transaction
/// (e.g., signatures, preimages).
#[derive(uniffi::Object, Clone, Default)]
pub struct SimplicityWitnessValues {
    inner: HashMap<String, SimplicityValue>,
}

impl_value_builder!(SimplicityWitnessValues);

impl SimplicityWitnessValues {
    fn to_inner(&self) -> simplicityhl::WitnessValues {
        simplicityhl::WitnessValues::from(convert_values_to_map(&self.inner))
    }
}

/// The result of running a Simplicity program.
///
/// Contains the pruned program (with witness) and the resulting value.
#[derive(uniffi::Object)]
pub struct SimplicityRunResult {
    pruned: Arc<simplicityhl::simplicity::RedeemNode<simplicityhl::simplicity::jet::Elements>>,
    value: simplicityhl::simplicity::Value,
}

#[uniffi::export]
impl SimplicityRunResult {
    /// Get the serialized program bytes.
    pub fn program_bytes(&self) -> Hex {
        let (program_bytes, _) = self.pruned.to_vec_with_witness();
        Hex::from(program_bytes)
    }

    /// Get the serialized witness bytes.
    pub fn witness_bytes(&self) -> Hex {
        let (_, witness_bytes) = self.pruned.to_vec_with_witness();
        Hex::from(witness_bytes)
    }

    /// Get the CMR (Commitment Merkle Root) of the pruned program.
    pub fn cmr(&self) -> Hex {
        let cmr = self.pruned.cmr();
        Hex::from(cmr.as_ref().to_vec())
    }

    /// Get the resulting value as a string representation.
    pub fn value(&self) -> String {
        format!("{:?}", self.value)
    }
}

/// A compiled Simplicity program ready for use in transactions.
#[derive(uniffi::Object)]
pub struct SimplicityProgram {
    inner: simplicityhl::CompiledProgram,
}

#[uniffi::export]
impl SimplicityProgram {
    /// Load and compile a Simplicity program from source.
    ///
    /// # Arguments
    /// * `source` - The Simplicity source code
    /// * `arguments` - Compile-time arguments to substitute into the program
    ///
    /// # Returns
    /// A compiled program ready for address generation and transaction signing.
    #[uniffi::constructor]
    pub fn load(source: String, arguments: &SimplicityArguments) -> Result<Arc<Self>, LwkError> {
        let compiled = scripts::load_program(&source, arguments.to_inner())?;
        Ok(Arc::new(SimplicityProgram { inner: compiled }))
    }

    /// Get the Commitment Merkle Root (CMR) of the program as hex.
    pub fn cmr(&self) -> Hex {
        let cmr = self.inner.commit().cmr();
        Hex::from(cmr.as_ref().to_vec())
    }

    /// Create a P2TR (Pay-to-Taproot) address for this Simplicity program.
    ///
    /// # Arguments
    /// * `internal_key` - The x-only public key
    /// * `network` - The network for address encoding
    ///
    /// # Returns
    /// The P2TR address that locks funds to this program.
    pub fn create_p2tr_address(
        &self,
        internal_key: &XOnlyPublicKey,
        network: &Network,
    ) -> Result<Arc<Address>, LwkError> {
        let x_only_key = internal_key.to_simplicityhl()?;

        let params = network_to_address_params(network.into());
        let cmr = self.inner.commit().cmr();
        let address = scripts::create_p2tr_address(cmr, &x_only_key, params);

        Ok(Arc::new(address.into()))
    }

    /// Get the taproot control block for script-path spending.
    ///
    /// # Arguments
    /// * `internal_key` - The x-only public key
    ///
    /// # Returns
    /// The serialized control block as hex.
    pub fn control_block(&self, internal_key: &XOnlyPublicKey) -> Result<Hex, LwkError> {
        let x_only_key = internal_key.to_simplicityhl()?;

        let cmr = self.inner.commit().cmr();
        let control_block = scripts::control_block(cmr, x_only_key);

        Ok(Hex::from(control_block.serialize()))
    }

    /// Get the sighash_all message for signing a Simplicity program input.
    ///
    /// # Arguments
    /// * `tx` - The transaction to sign
    /// * `program_public_key` - The x-only public key used in the address
    /// * `utxos` - The UTXOs being spent (in input order)
    /// * `input_index` - The index of the input being signed
    /// * `network` - The network
    /// * `genesis_hash` - The genesis block hash (32 bytes hex)
    ///
    /// # Returns
    /// The 32-byte message hash to sign.
    pub fn get_sighash_all(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        network: &Network,
        genesis_hash: Hex,
    ) -> Result<Hex, LwkError> {
        let x_only_key = program_public_key.to_simplicityhl()?;

        let genesis = get_genesis_hash(&genesis_hash)?;
        let params = network_to_address_params(network.into());
        let utxos_inner = convert_utxos(&utxos);

        let message = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            input_index as usize,
            params,
            genesis,
        )?;

        Ok(Hex::from(message.as_ref().to_vec()))
    }

    /// Finalize a transaction with a Simplicity witness for the specified input.
    ///
    /// # Arguments
    /// * `tx` - The transaction to finalize
    /// * `program_public_key` - The x-only public key used in the address
    /// * `utxos` - The UTXOs being spent (in input order)
    /// * `input_index` - The index of the input being finalized
    /// * `witness_values` - Runtime witness values (e.g., signatures)
    /// * `network` - The network
    /// * `genesis_hash` - The genesis block hash (32 bytes hex)
    /// * `log_level` - Simplicity execution tracing level
    ///
    /// # Returns
    /// The finalized transaction with the Simplicity witness attached.
    #[allow(clippy::too_many_arguments)]
    pub fn finalize_transaction(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        genesis_hash: Hex,
        log_level: SimplicityLogLevel,
    ) -> Result<Arc<Transaction>, LwkError> {
        let x_only_key = program_public_key.to_simplicityhl()?;

        let genesis = get_genesis_hash(&genesis_hash)?;
        let params = network_to_address_params(network.into());
        let utxos_inner = convert_utxos(&utxos);

        let finalized = signer::finalize_transaction(
            tx.as_ref().clone(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            input_index as usize,
            witness_values.to_inner(),
            params,
            genesis,
            log_level.into(),
        )?;

        Ok(Arc::new(finalized.into()))
    }

    /// Create a Schnorr signature for a P2PK Simplicity program input.
    ///
    /// This method computes the sighash_all and signs it with a key derived from the signer.
    /// The resulting signature can be used as a witness value for transaction finalization.
    ///
    /// # Arguments
    /// * `signer` - The software signer containing the master key
    /// * `derivation_path` - The BIP32 derivation path for the signing key (e.g., "m/86'/1'/0'/0/0")
    /// * `tx` - The transaction to sign
    /// * `utxos` - The UTXOs being spent (in input order)
    /// * `input_index` - The index of the input being signed
    /// * `network` - The network
    /// * `genesis_hash` - The genesis block hash (32 bytes hex)
    ///
    /// # Returns
    /// The 64-byte Schnorr signature as hex.
    #[allow(clippy::too_many_arguments)]
    pub fn create_p2pk_signature(
        &self,
        signer: &crate::Signer,
        derivation_path: String,
        tx: &Transaction,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        network: &Network,
        genesis_hash: Hex,
    ) -> Result<Hex, LwkError> {
        let keypair = derive_keypair(signer, &derivation_path)?;
        let x_only_pubkey = keypair.x_only_public_key().0;

        let genesis = get_genesis_hash(&genesis_hash)?;
        let params = network_to_address_params(network.into());
        let utxos_inner = convert_utxos(&utxos);

        let sighash = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &x_only_pubkey,
            &utxos_inner,
            input_index as usize,
            params,
            genesis,
        )?;

        let signature = keypair.sign_schnorr(sighash);

        Ok(Hex::from(signature.serialize().to_vec()))
    }

    /// Satisfy and execute this program in a transaction environment.
    ///
    /// Returns the pruned program and the resulting value.
    ///
    /// # Arguments
    /// * `tx` - The transaction
    /// * `program_public_key` - The x-only public key used in the address
    /// * `utxos` - The UTXOs being spent (in input order)
    /// * `input_index` - The index of the input
    /// * `witness_values` - Runtime witness values (e.g., signatures)
    /// * `network` - The network
    /// * `genesis_hash` - The genesis block hash (32 bytes hex)
    /// * `log_level` - Simplicity execution tracing level
    ///
    /// # Returns
    /// The run result containing the pruned program and resulting value.
    ///
    /// # Errors
    /// Returns error if witness satisfaction or program execution fails.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        genesis_hash: Hex,
        log_level: SimplicityLogLevel,
    ) -> Result<Arc<SimplicityRunResult>, LwkError> {
        let x_only_key = program_public_key.to_simplicityhl()?;

        let genesis = get_genesis_hash(&genesis_hash)?;
        let params = network_to_address_params(network.into());
        let utxos_inner = convert_utxos(&utxos);

        let env = signer::get_and_verify_env(
            tx.as_ref(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            params,
            genesis,
            input_index as usize,
        )?;

        let (pruned, value) = runner::run_program(
            &self.inner,
            witness_values.to_inner(),
            &env,
            log_level.into(),
        )?;

        Ok(Arc::new(SimplicityRunResult { pruned, value }))
    }
}

/// Get the x-only public key for a given derivation path from a signer.
///
/// This is useful for creating Simplicity P2PK addresses and programs.
///
/// # Arguments
/// * `signer` - The software signer containing the master key
/// * `derivation_path` - The BIP32 derivation path (e.g., "m/86'/1'/0'/0/0")
///
/// # Returns
/// The x-only public key.
#[uniffi::export]
pub fn simplicity_derive_xonly_pubkey(
    signer: &crate::Signer,
    derivation_path: String,
) -> Result<Arc<XOnlyPublicKey>, LwkError> {
    let keypair = derive_keypair(signer, &derivation_path)?;
    Ok(XOnlyPublicKey::from_keypair(&keypair))
}

fn get_genesis_hash(genesis_hash: &Hex) -> Result<simplicityhl::elements::BlockHash, LwkError> {
    parse_genesis_hash(genesis_hash.as_ref()).map_err(|msg| LwkError::Generic {
        msg: msg.to_string(),
    })
}

fn convert_utxos(utxos: &[Arc<TxOut>]) -> Vec<elements::TxOut> {
    utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect()
}

fn derive_keypair(
    signer: &crate::Signer,
    derivation_path: &str,
) -> Result<elements::bitcoin::secp256k1::Keypair, LwkError> {
    let path = DerivationPath::from_str(derivation_path).map_err(|e| LwkError::Generic {
        msg: format!("Invalid derivation path: {e}"),
    })?;

    let derived_xprv = signer.inner.derive_xprv(&path)?;
    Ok(elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derived_xprv.private_key,
    ))
}
