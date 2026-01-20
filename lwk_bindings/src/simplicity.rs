use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::bip32::DerivationPath;
use lwk_simplicity_options::scripts;
use lwk_simplicity_options::signer;
use lwk_simplicity_options::simplicityhl;
use lwk_simplicity_options::utils::{
    convert_values_to_map, network_to_address_params, parse_genesis_hash, validate_bytes_length,
    SimplicityValue,
};

use crate::blockdata::tx_out::TxOut;
use crate::types::Hex;
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

/// A compiled Simplicity program ready for use in transactions.
#[derive(uniffi::Object)]
pub struct SimplicityProgram {
    inner: simplicityhl::CompiledProgram,
}

#[uniffi::export]
impl SimplicityProgram {
    /// Get the Commitment Merkle Root (CMR) of the program as hex.
    pub fn cmr(&self) -> Hex {
        let cmr = self.inner.commit().cmr();
        Hex::from(cmr.as_ref().to_vec())
    }
}

/// Load and compile a Simplicity program from source.
///
/// # Arguments
/// * `source` - The Simplicity source code
/// * `arguments` - Compile-time arguments to substitute into the program
///
/// # Returns
/// A compiled program ready for address generation and transaction signing.
#[uniffi::export]
pub fn simplicity_load_program(
    source: String,
    arguments: &SimplicityArguments,
) -> Result<Arc<SimplicityProgram>, LwkError> {
    let compiled = scripts::load_program(&source, arguments.to_inner())?;
    Ok(Arc::new(SimplicityProgram { inner: compiled }))
}

/// Create a P2TR (Pay-to-Taproot) address for a Simplicity program.
///
/// # Arguments
/// * `program` - The compiled Simplicity program
/// * `internal_key` - The x-only public key (32 bytes hex)
/// * `network` - The network for address encoding
///
/// # Returns
/// The P2TR address that locks funds to this program.
// TODO(KyrylR): Add a proper XOnlyPublicKey type in lwk_bindings instead of using Hex.
#[uniffi::export]
pub fn simplicity_create_p2tr_address(
    program: &SimplicityProgram,
    internal_key: Hex,
    network: &Network,
) -> Result<Arc<Address>, LwkError> {
    let key_bytes: [u8; 32] = internal_key
        .as_ref()
        .try_into()
        .map_err(|_| LwkError::Generic {
            msg: "internal_key must be exactly 32 bytes".to_string(),
        })?;

    let x_only_key = simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(&key_bytes)
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid x-only public key: {e}"),
        })?;

    let params = network_to_address_params(network.into());
    let cmr = program.inner.commit().cmr();
    let address = scripts::create_p2tr_address(cmr, &x_only_key, params);

    Ok(Arc::new(address.into()))
}

/// Get the taproot control block for script-path spending.
///
/// # Arguments
/// * `program` - The compiled Simplicity program
/// * `internal_key` - The x-only public key (32 bytes hex)
///
/// # Returns
/// The serialized control block as hex.
#[uniffi::export]
pub fn simplicity_control_block(
    program: &SimplicityProgram,
    internal_key: Hex,
) -> Result<Hex, LwkError> {
    let key_bytes: [u8; 32] = internal_key
        .as_ref()
        .try_into()
        .map_err(|_| LwkError::Generic {
            msg: "internal_key must be exactly 32 bytes".to_string(),
        })?;

    let x_only_key = simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(&key_bytes)
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid x-only public key: {e}"),
        })?;

    let cmr = program.inner.commit().cmr();
    let control_block = scripts::control_block(cmr, x_only_key);

    Ok(Hex::from(control_block.serialize()))
}

/// Get the sighash_all message for signing a Simplicity program input.
///
/// # Arguments
/// * `tx` - The transaction to sign
/// * `program` - The compiled Simplicity program
/// * `program_public_key` - The x-only public key used in the address (32 bytes hex)
/// * `utxos` - The UTXOs being spent (in input order)
/// * `input_index` - The index of the input being signed
/// * `network` - The network
/// * `genesis_hash` - The genesis block hash (32 bytes hex)
///
/// # Returns
/// The 32-byte message hash to sign.
#[uniffi::export]
pub fn simplicity_get_sighash_all(
    tx: &Transaction,
    program: &SimplicityProgram,
    program_public_key: Hex,
    utxos: Vec<Arc<TxOut>>,
    input_index: u32,
    network: &Network,
    genesis_hash: Hex,
) -> Result<Hex, LwkError> {
    let key_bytes: [u8; 32] =
        program_public_key
            .as_ref()
            .try_into()
            .map_err(|_| LwkError::Generic {
                msg: "program_public_key must be exactly 32 bytes".to_string(),
            })?;

    let x_only_key = simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(&key_bytes)
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid x-only public key: {e}"),
        })?;

    let genesis = get_genesis_hash(&genesis_hash)?;
    let params = network_to_address_params(network.into());

    let utxos_inner: Vec<elements::TxOut> = utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect();

    let message = signer::get_sighash_all(
        tx.as_ref(),
        &program.inner,
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
/// * `program` - The compiled Simplicity program
/// * `program_public_key` - The x-only public key used in the address (32 bytes hex)
/// * `utxos` - The UTXOs being spent (in input order)
/// * `input_index` - The index of the input being finalized
/// * `witness_values` - Runtime witness values (e.g., signatures)
/// * `network` - The network
/// * `genesis_hash` - The genesis block hash (32 bytes hex)
/// * `log_level` - Simplicity execution tracing level
///
/// # Returns
/// The finalized transaction with the Simplicity witness attached.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn simplicity_finalize_transaction(
    tx: &Transaction,
    program: &SimplicityProgram,
    program_public_key: Hex,
    utxos: Vec<Arc<TxOut>>,
    input_index: u32,
    witness_values: &SimplicityWitnessValues,
    network: &Network,
    genesis_hash: Hex,
    log_level: SimplicityLogLevel,
) -> Result<Arc<Transaction>, LwkError> {
    let key_bytes: [u8; 32] =
        program_public_key
            .as_ref()
            .try_into()
            .map_err(|_| LwkError::Generic {
                msg: "program_public_key must be exactly 32 bytes".to_string(),
            })?;

    let x_only_key = simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(&key_bytes)
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid x-only public key: {e}"),
        })?;

    let genesis = get_genesis_hash(&genesis_hash)?;
    let params = network_to_address_params(network.into());

    let utxos_inner: Vec<elements::TxOut> = utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect();

    let finalized = signer::finalize_transaction(
        tx.as_ref().clone(),
        &program.inner,
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
/// This function computes the sighash_all and signs it with a key derived from the signer.
/// The resulting signature can be used as a witness value for transaction finalization.
///
/// # Arguments
/// * `signer` - The software signer containing the master key
/// * `derivation_path` - The BIP32 derivation path for the signing key (e.g., "m/86'/1'/0'/0/0")
/// * `tx` - The transaction to sign
/// * `program` - The compiled Simplicity program
/// * `utxos` - The UTXOs being spent (in input order)
/// * `input_index` - The index of the input being signed
/// * `network` - The network
/// * `genesis_hash` - The genesis block hash (32 bytes hex)
///
/// # Returns
/// The 64-byte Schnorr signature as hex.
#[uniffi::export]
#[allow(clippy::too_many_arguments)]
pub fn simplicity_create_p2pk_signature(
    signer: &crate::Signer,
    derivation_path: String,
    tx: &Transaction,
    program: &SimplicityProgram,
    utxos: Vec<Arc<TxOut>>,
    input_index: u32,
    network: &Network,
    genesis_hash: Hex,
) -> Result<Hex, LwkError> {
    let path = DerivationPath::from_str(&derivation_path).map_err(|e| LwkError::Generic {
        msg: format!("Invalid derivation path: {e}"),
    })?;

    let derived_xprv = signer.inner.derive_xprv(&path)?;
    let keypair = elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derived_xprv.private_key,
    );
    let x_only_pubkey = keypair.x_only_public_key().0;

    let genesis = get_genesis_hash(&genesis_hash)?;
    let params = network_to_address_params(network.into());

    let utxos_inner: Vec<elements::TxOut> = utxos
        .iter()
        .map(|u| elements::TxOut::from(u.as_ref()))
        .collect();

    let sighash = signer::get_sighash_all(
        tx.as_ref(),
        &program.inner,
        &x_only_pubkey,
        &utxos_inner,
        input_index as usize,
        params,
        genesis,
    )?;

    let signature = keypair.sign_schnorr(sighash);

    Ok(Hex::from(signature.serialize().to_vec()))
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
/// The 32-byte x-only public key as hex.
#[uniffi::export]
pub fn simplicity_derive_xonly_pubkey(
    signer: &crate::Signer,
    derivation_path: String,
) -> Result<Hex, LwkError> {
    let path = DerivationPath::from_str(&derivation_path).map_err(|e| LwkError::Generic {
        msg: format!("Invalid derivation path: {e}"),
    })?;

    let derived_xprv = signer.inner.derive_xprv(&path)?;
    let keypair = elements::bitcoin::secp256k1::Keypair::from_secret_key(
        elements::bitcoin::secp256k1::SECP256K1,
        &derived_xprv.private_key,
    );

    let (xonly, _parity) = keypair.x_only_public_key();

    Ok(Hex::from(xonly.serialize().to_vec()))
}

fn get_genesis_hash(genesis_hash: &Hex) -> Result<simplicityhl::elements::BlockHash, LwkError> {
    parse_genesis_hash(genesis_hash.as_ref()).map_err(|msg| LwkError::Generic {
        msg: msg.to_string(),
    })
}
