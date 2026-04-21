use std::io;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use crate::{Bip, DescriptorBlindingKey, LwkError, Network, Pset, Singlesig, WolletDescriptor};

/// Callback interface for Android-owned Jade byte transports.
#[uniffi::export(with_foreign)]
pub trait JadeTransportCallbacks: Send + Sync {
    /// Write all bytes to the Jade transport or return an error.
    fn write(&self, bytes: Vec<u8>) -> Result<(), LwkError>;

    /// Read up to `max_len` bytes from the Jade transport.
    fn read(&self, max_len: u32) -> Result<Vec<u8>, LwkError>;
}

/// A Jade hardware wallet connected through the blocking Rust Jade client.
#[derive(uniffi::Object)]
pub struct Jade {
    inner: lwk_jade::Jade,
}

#[uniffi::export]
impl Jade {
    /// Connect to a Jade-compatible socket endpoint.
    #[uniffi::constructor]
    pub fn from_socket(host: &str, port: u16, network: &Network) -> Result<Arc<Self>, LwkError> {
        let socket = format!("{host}:{port}")
            .to_socket_addrs()
            .map_err(|error| LwkError::from(format!("{error}")))?
            .next()
            .ok_or_else(|| LwkError::from("socket address did not resolve"))?;
        let inner = lwk_jade::Jade::from_socket(socket, network.into())?;
        Ok(Arc::new(Self { inner }))
    }

    /// Connect to a Jade device through a foreign blocking byte transport.
    #[uniffi::constructor]
    pub fn from_transport(
        transport: Arc<dyn JadeTransportCallbacks>,
        network: &Network,
    ) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_jade::Jade::from_transport(
            Box::new(ForeignJadeTransport {
                callbacks: transport,
            }),
            network.into(),
        );
        Ok(Arc::new(Self { inner }))
    }

    /// Sign the given `pset`, returning a new PSET with Jade signatures added.
    pub fn sign(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        lwk_common::Signer::sign(&self.inner, &mut pset)?;
        Ok(Arc::new(pset.into()))
    }

    /// Return the signer fingerprint.
    pub fn fingerprint(&self) -> Result<String, LwkError> {
        Ok(lwk_common::Signer::fingerprint(&self.inner)?.to_string())
    }

    /// Return keyorigin and xpub, like "[73c5da0a/84h/1h/0h]tpub...".
    pub fn keyorigin_xpub(&self, bip: &Bip) -> Result<String, LwkError> {
        Ok(lwk_common::Signer::keyorigin_xpub(
            &self.inner,
            bip.inner(),
            self.inner.network().is_mainnet(),
        )?)
    }

    /// Return the witness public key hash, slip77 descriptor of this Jade.
    pub fn wpkh_slip77_descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        self.singlesig_desc(Singlesig::Wpkh, DescriptorBlindingKey::Slip77)
    }

    /// Generate a singlesig descriptor with the given parameters.
    pub fn singlesig_desc(
        &self,
        script_variant: Singlesig,
        blinding_variant: DescriptorBlindingKey,
    ) -> Result<Arc<WolletDescriptor>, LwkError> {
        let desc_str =
            lwk_common::singlesig_desc(&self.inner, script_variant.into(), blinding_variant.into())
                .map_err(LwkError::from)?;
        WolletDescriptor::new(&desc_str)
    }
}

struct ForeignJadeTransport {
    callbacks: Arc<dyn JadeTransportCallbacks>,
}

impl lwk_jade::JadeTransport for ForeignJadeTransport {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.callbacks
            .write(bytes.to_vec())
            .map_err(callback_io_error)
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let max_len = u32::try_from(buf.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Jade transport read buffer is too large",
            )
        })?;
        let bytes = self.callbacks.read(max_len).map_err(callback_io_error)?;
        if bytes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Jade transport returned no bytes",
            ));
        }
        if bytes.len() > buf.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Jade transport returned more bytes than requested",
            ));
        }

        buf[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

fn callback_io_error(error: LwkError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error.to_string())
}

#[cfg(test)]
mod transport_tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    use elements::Txid;
    use lwk_containers::testcontainers::clients;
    use lwk_jade::TestJadeEmulator;
    use lwk_test_util::TEST_MNEMONIC;
    use lwk_wollet::blocking::BlockchainBackend;
    use lwk_wollet::{ElectrumClient, ElectrumUrl, ElementsNetwork, Wollet, WolletBuilder};

    use super::{Jade, JadeTransportCallbacks};
    use crate::{LwkError, Network, Pset};

    #[test]
    fn jade_from_transport_signs_wallet_pset() {
        let env = lwk_test_util::TestEnvBuilder::from_env()
            .with_electrum()
            .build();
        let network = ElementsNetwork::default_regtest();
        let binding_network: Network = network.into();
        let mut client = electrum_client(&env);

        let docker = clients::Cli::default();
        let mut emulator = TestJadeEmulator::new(&docker);
        emulator.set_debug_mnemonic(TEST_MNEMONIC);
        let port = emulator.port();
        drop(emulator.jade);

        let transport = Arc::new(TcpJadeTransport::connect(port));
        let jade = Jade::from_transport(transport, &binding_network).unwrap();
        let jade_fingerprint = jade.fingerprint().unwrap();
        let jade_descriptor = jade.wpkh_slip77_descriptor().unwrap();
        let mut jade_wollet = WolletBuilder::new(network, jade_descriptor.as_ref().into())
            .build()
            .unwrap();

        let wallet_funding_address = jade_wollet.address(Some(0)).unwrap();
        let wallet_funding_txid =
            env.elementsd_sendtoaddress(wallet_funding_address.address(), 10_000, None);
        env.elementsd_generate(1);
        wait_for_tx(&mut jade_wollet, &mut client, &wallet_funding_txid);
        assert_eq!(jade_wollet.utxos().unwrap().len(), 1);

        let node_address = env.elementsd_getnewaddress().to_unconfidential();
        let pset = jade_wollet
            .tx_builder()
            .add_explicit_recipient(&node_address, 1_000, jade_wollet.policy_asset())
            .unwrap()
            .finish()
            .unwrap();
        assert_eq!(pset.inputs().len(), 1);
        assert!(pset.inputs()[0].partial_sigs.is_empty());
        assert!(has_jade_derivation(&pset.inputs()[0], &jade_fingerprint));

        let signed = jade.sign(&Pset::from(pset)).unwrap();
        let signed_inner = signed.inner();
        assert_eq!(signed_inner.inputs()[0].partial_sigs.len(), 1);
        let total_partial_sigs: usize = signed_inner
            .inputs()
            .iter()
            .map(|input| input.partial_sigs.len())
            .sum();
        assert_eq!(total_partial_sigs, 1);
    }

    struct TcpJadeTransport {
        stream: Mutex<TcpStream>,
    }

    impl TcpJadeTransport {
        fn connect(port: u16) -> Self {
            let stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
            Self {
                stream: Mutex::new(stream),
            }
        }
    }

    impl JadeTransportCallbacks for TcpJadeTransport {
        fn write(&self, bytes: Vec<u8>) -> Result<(), LwkError> {
            self.stream
                .lock()
                .unwrap()
                .write_all(&bytes)
                .map_err(|error| LwkError::from(error.to_string()))
        }

        fn read(&self, max_len: u32) -> Result<Vec<u8>, LwkError> {
            let max_len = usize::try_from(max_len).map_err(|error| {
                LwkError::from(format!("invalid Jade transport read length: {error}"))
            })?;
            let mut bytes = vec![0; max_len];
            let len = self
                .stream
                .lock()
                .unwrap()
                .read(&mut bytes)
                .map_err(|error| LwkError::from(error.to_string()))?;
            bytes.truncate(len);
            Ok(bytes)
        }
    }

    fn has_jade_derivation(input: &elements::pset::Input, jade_fingerprint: &str) -> bool {
        input
            .bip32_derivation
            .values()
            .any(|(fingerprint, _)| fingerprint.to_string() == jade_fingerprint)
    }

    fn electrum_client(env: &lwk_test_util::TestEnv) -> ElectrumClient {
        let electrum_url = ElectrumUrl::from_str(&env.electrum_url()).unwrap();
        ElectrumClient::new(&electrum_url).unwrap()
    }

    fn sync<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S) {
        let update = client.full_scan(wollet).unwrap();
        if let Some(update) = update {
            wollet.apply_update(update).unwrap();
        }
    }

    fn wait_for_tx<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S, txid: &Txid) {
        for _ in 0..120 {
            sync(wollet, client);
            let list = wollet.transactions().unwrap();
            if list.iter().any(|entry| &entry.txid == txid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        panic!("Wallet does not have {txid} in its list");
    }
}

#[cfg(all(test, feature = "simplicity"))]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;

    use elements::pset::PartiallySignedTransaction;
    use elements::Txid;
    use lwk_containers::testcontainers::clients;
    use lwk_jade::TestJadeEmulator;
    use lwk_simplicity::scripts::{create_p2tr_address, load_program};
    use lwk_simplicity::simplicityhl::{
        num::U256, str::WitnessName, value::ValueConstructible, Arguments, Value,
    };
    use lwk_test_util::TEST_MNEMONIC;
    use lwk_wollet::bitcoin::secp256k1::{Keypair, SecretKey};
    use lwk_wollet::blocking::BlockchainBackend;
    use lwk_wollet::elements::hex::ToHex;
    use lwk_wollet::{
        ElectrumClient, ElectrumUrl, ElementsNetwork, Wollet, WolletBuilder, WolletDescriptor, EC,
    };

    use super::Jade;
    use crate::{Network, Pset};

    const P2PK_SOURCE: &str = include_str!("../../lwk_simplicity/data/p2pk.simf");

    #[test]
    fn jade_signs_explicit_wallet_input_and_skips_simplicity_input() {
        let env = lwk_test_util::TestEnvBuilder::from_env()
            .with_electrum()
            .build();
        let network = ElementsNetwork::default_regtest();
        let binding_network: Network = network.into();
        let mut client = electrum_client(&env);

        let docker = clients::Cli::default();
        let mut emulator = TestJadeEmulator::new(&docker);
        emulator.set_debug_mnemonic(TEST_MNEMONIC);
        let port = emulator.port();
        drop(emulator.jade);

        let jade = Jade::from_socket("127.0.0.1", port, &binding_network).unwrap();
        let jade_fingerprint = jade.fingerprint().unwrap();
        let jade_descriptor = jade.wpkh_slip77_descriptor().unwrap();
        let mut jade_wollet = WolletBuilder::new(network, jade_descriptor.as_ref().into())
            .build()
            .unwrap();

        let wallet_funding_address = jade_wollet.address(Some(0)).unwrap();
        let explicit_wallet_address = wallet_funding_address.address().to_unconfidential();
        let wallet_funding_txid =
            env.elementsd_sendtoaddress(&explicit_wallet_address, 10_000, None);
        env.elementsd_generate(1);
        wait_for_tx(&mut jade_wollet, &mut client, &wallet_funding_txid);
        let jade_explicit_utxos = jade_wollet.explicit_utxos().unwrap();
        assert_eq!(jade_explicit_utxos.len(), 1);

        let (simplicity_address, mut simplicity_wollet) = simplicity_p2pk_wallet(network);
        let asset_amount = 1;
        let asset = env.elementsd_issueasset(asset_amount);
        let simplicity_funding_txid =
            env.elementsd_sendtoaddress(&simplicity_address, asset_amount, Some(asset));
        env.elementsd_generate(1);
        wait_for_tx(
            &mut simplicity_wollet,
            &mut client,
            &simplicity_funding_txid,
        );
        let simplicity_utxos = simplicity_wollet.explicit_utxos().unwrap();
        assert_eq!(simplicity_utxos.len(), 1);

        let mut external_utxos = simplicity_utxos;
        external_utxos.extend(jade_explicit_utxos);

        let node_address = env.elementsd_getnewaddress().to_unconfidential();
        let pset = jade_wollet
            .tx_builder()
            .add_explicit_recipient(&node_address, asset_amount, asset)
            .unwrap()
            .add_external_utxos(external_utxos)
            .unwrap()
            .finish()
            .unwrap();
        assert_eq!(pset.inputs().len(), 2);

        let wallet_input_index = wallet_input_index(&pset, &jade_fingerprint);
        let simplicity_input_index = simplicity_input_index(&pset, &jade_fingerprint);
        assert_ne!(wallet_input_index, simplicity_input_index);
        assert!(is_explicit_input(&pset.inputs()[wallet_input_index]));

        for input in pset.inputs() {
            assert!(input.partial_sigs.is_empty());
        }
        assert!(pset
            .outputs()
            .iter()
            .any(|output| is_explicit_normal_output(output)));
        assert!(!has_jade_derivation(
            &pset.inputs()[simplicity_input_index],
            &jade_fingerprint
        ));

        let binding_pset = Pset::from(pset.clone());
        let signed = jade.sign(&binding_pset).unwrap();
        let signed_inner = signed.inner();

        assert_ne!(pset.to_string(), signed_inner.to_string());
        assert_eq!(
            pset_without_partial_sigs(pset.clone()),
            pset_without_partial_sigs(signed_inner.clone())
        );
        assert_eq!(
            signed_inner.inputs()[wallet_input_index].partial_sigs.len(),
            1
        );
        assert!(signed_inner.inputs()[simplicity_input_index]
            .partial_sigs
            .is_empty());
        let total_partial_sigs: usize = signed_inner
            .inputs()
            .iter()
            .map(|input| input.partial_sigs.len())
            .sum();
        assert_eq!(total_partial_sigs, 1);
    }

    fn simplicity_p2pk_wallet(network: ElementsNetwork) -> (elements::Address, lwk_wollet::Wollet) {
        let secret_key = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&EC, &secret_key);
        let (xonly, _) = keypair.x_only_public_key();

        let mut args = HashMap::new();
        args.insert(
            WitnessName::from_str_unchecked("PUBLIC_KEY"),
            Value::u256(U256::from_byte_array(xonly.serialize())),
        );
        let program = load_program(P2PK_SOURCE, Arguments::from(args)).unwrap();
        let address = create_p2tr_address(program.commit().cmr(), &xonly, network.address_params());
        let descriptor =
            WolletDescriptor::from_str(&format!(":{}", address.script_pubkey().to_hex())).unwrap();
        let wollet = WolletBuilder::new(network, descriptor).build().unwrap();

        (address, wollet)
    }

    fn wallet_input_index(pset: &PartiallySignedTransaction, jade_fingerprint: &str) -> usize {
        pset.inputs()
            .iter()
            .position(|input| {
                has_jade_derivation(input, jade_fingerprint)
                    && input
                        .witness_utxo
                        .as_ref()
                        .map(|utxo| utxo.script_pubkey.is_v0_p2wpkh())
                        .unwrap_or(false)
            })
            .expect("wallet input with Jade derivation")
    }

    fn simplicity_input_index(pset: &PartiallySignedTransaction, jade_fingerprint: &str) -> usize {
        pset.inputs()
            .iter()
            .position(|input| {
                !has_jade_derivation(input, jade_fingerprint)
                    && input
                        .witness_utxo
                        .as_ref()
                        .map(|utxo| is_p2tr_script(&utxo.script_pubkey))
                        .unwrap_or(false)
            })
            .expect("simplicity p2tr input without Jade derivation")
    }

    fn has_jade_derivation(input: &elements::pset::Input, jade_fingerprint: &str) -> bool {
        input
            .bip32_derivation
            .values()
            .any(|(fingerprint, _)| fingerprint.to_string() == jade_fingerprint)
    }

    fn is_p2tr_script(script: &elements::Script) -> bool {
        let bytes = script.as_bytes();
        bytes.len() == 34 && bytes[0] == 0x51 && bytes[1] == 0x20
    }

    fn is_explicit_input(input: &elements::pset::Input) -> bool {
        input
            .witness_utxo
            .as_ref()
            .map(|utxo| matches!(utxo.value, elements::confidential::Value::Explicit(_)))
            .unwrap_or(false)
    }

    fn is_explicit_normal_output(output: &elements::pset::Output) -> bool {
        !output.script_pubkey.is_empty()
            && output.script_pubkey != lwk_common::burn_script()
            && output.blinding_key.is_none()
            && output.asset_comm.is_none()
            && output.amount_comm.is_none()
            && output.blind_asset_proof.is_none()
            && output.blind_value_proof.is_none()
    }

    fn pset_without_partial_sigs(
        mut pset: PartiallySignedTransaction,
    ) -> PartiallySignedTransaction {
        for input in pset.inputs_mut() {
            input.partial_sigs.clear();
        }
        pset
    }

    fn electrum_client(env: &lwk_test_util::TestEnv) -> ElectrumClient {
        let electrum_url = ElectrumUrl::from_str(&env.electrum_url()).unwrap();
        ElectrumClient::new(&electrum_url).unwrap()
    }

    fn sync<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S) {
        let update = client.full_scan(wollet).unwrap();
        if let Some(update) = update {
            wollet.apply_update(update).unwrap();
        }
    }

    fn wait_for_tx<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S, txid: &Txid) {
        for _ in 0..120 {
            sync(wollet, client);
            let list = wollet.transactions().unwrap();
            if list.iter().any(|entry| &entry.txid == txid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        panic!("Wallet does not have {txid} in its list");
    }
}
