//! Pegin related types and functions.
//!
//! A Peg-in is a way to convert bitcoin (BTC) on the mainchain to liquid bitcoin (L-BTC).

use elements::{bitcoin, BlockHeader};
use elements_miniscript::{BtcDescriptor, BtcMiniscript, BtcSegwitv0};

use crate::{Error, Network};

/// The Bitcoin address type used for a pegin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeginAddressType {
    /// Native pay-to-witness-script-hash.
    P2wsh,
    /// Pay-to-script-hash wrapping pay-to-witness-script-hash.
    P2shP2wsh,
}

/// A snapshot of federation pegin parameters and their validity window.
///
/// A `FedPeg` is created from a full dynafed block header. It retains the exact
/// federation script and program used to derive pegin addresses, so callers do
/// not need to pass an unversioned federation descriptor around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FedPeg {
    network: Network,
    script: bitcoin::ScriptBuf,
    program: bitcoin::ScriptBuf,
    descriptor: BtcDescriptor<bitcoin::PublicKey>,
    epoch: u32,
    epoch_start_height: u32,
    valid_from_height: u32,
    valid_until_height: u32,
    address_type: PeginAddressType,
}

impl FedPeg {
    /// Create a federation peg snapshot from a full dynafed block header.
    ///
    /// The validity window uses the epoch parameters defined by `network`.
    /// Use [`FedPeg::from_block_header_with_params`] when a custom Elements
    /// chain overrides those defaults.
    pub fn from_block_header(network: Network, header: &BlockHeader) -> Result<Self, Error> {
        Self::from_block_header_with_params(
            network,
            header,
            network.dynamic_epoch_length(),
            network.total_valid_epochs(),
        )
    }

    /// Create a federation peg snapshot with explicit dynafed epoch parameters.
    ///
    /// This constructor is intended for custom Elements chains whose
    /// `dynamic_epoch_length` or `total_valid_epochs` differ from LWK's
    /// defaults.
    pub fn from_block_header_with_params(
        network: Network,
        header: &BlockHeader,
        epoch_length: u32,
        total_valid_epochs: u32,
    ) -> Result<Self, Error> {
        if epoch_length == 0 {
            return Err(Error::InvalidFedPeg(
                "dynamic epoch length must be greater than zero".to_string(),
            ));
        }
        if total_valid_epochs == 0 {
            return Err(Error::InvalidFedPeg(
                "total valid epochs must be greater than zero".to_string(),
            ));
        }

        let current = match &header.ext {
            elements::BlockExtData::Proof { .. } => {
                return Err(Error::InvalidFedPeg(
                    "block header does not contain dynafed parameters".to_string(),
                ));
            }
            elements::BlockExtData::Dynafed { current, .. } => current,
        };

        let program = current
            .fedpeg_program()
            .cloned()
            .ok_or_else(|| Error::InvalidFedPeg("dynafed parameters are not full".to_string()))?;
        let script = current
            .fedpegscript()
            .cloned()
            .map(bitcoin::ScriptBuf::from_bytes)
            .ok_or_else(|| Error::InvalidFedPeg("dynafed parameters are not full".to_string()))?;

        let address_type = classify_fedpeg_program(&program, &script)?;
        type Segwitv0Script = BtcMiniscript<bitcoin::PublicKey, BtcSegwitv0>;
        let miniscript = Segwitv0Script::parse(&script)
            .map_err(|e| Error::InvalidFedPeg(format!("invalid federation script: {e}")))?;
        let descriptor = BtcDescriptor::new_wsh(miniscript)
            .map_err(|e| Error::InvalidFedPeg(format!("invalid federation descriptor: {e}")))?;
        descriptor
            .sanity_check()
            .map_err(|e| Error::InvalidFedPeg(format!("unsafe federation descriptor: {e}")))?;

        let epoch = header.height / epoch_length;
        let epoch_start_height = epoch
            .checked_mul(epoch_length)
            .ok_or_else(|| Error::InvalidFedPeg("epoch start height overflow".to_string()))?;
        let valid_until_height = epoch
            .checked_add(total_valid_epochs)
            .and_then(|epoch| epoch.checked_mul(epoch_length))
            .and_then(|height| height.checked_sub(1))
            .ok_or_else(|| Error::InvalidFedPeg("validity window overflow".to_string()))?;

        Ok(Self {
            network,
            script,
            program,
            descriptor,
            epoch,
            epoch_start_height,
            valid_from_height: header.height,
            valid_until_height,
            address_type,
        })
    }

    /// Return the Elements network this snapshot belongs to.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Return the untweaked federation witness script.
    pub fn script(&self) -> &bitcoin::Script {
        &self.script
    }

    /// Return the script pubkey committing to the federation script.
    ///
    /// Its form determines whether pegin addresses use native P2WSH or
    /// P2SH-wrapped P2WSH.
    pub fn program(&self) -> &bitcoin::Script {
        &self.program
    }

    /// Return the zero-based dynafed epoch containing this snapshot.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Return the first block height of this dynafed epoch.
    pub fn epoch_start_height(&self) -> u32 {
        self.epoch_start_height
    }

    /// Return the first block height at which this snapshot is valid.
    pub fn valid_from_height(&self) -> u32 {
        self.valid_from_height
    }

    /// Return the last block height at which this snapshot is valid.
    pub fn valid_until_height(&self) -> u32 {
        self.valid_until_height
    }

    /// Return the inclusive Liquid block-height validity window.
    pub fn validity_window(&self) -> std::ops::RangeInclusive<u32> {
        self.valid_from_height..=self.valid_until_height
    }

    /// Return whether this snapshot is valid at `height`.
    pub fn is_valid_at(&self, height: u32) -> bool {
        self.validity_window().contains(&height)
    }

    /// Return the Bitcoin address type required by this federation snapshot.
    pub fn address_type(&self) -> PeginAddressType {
        self.address_type
    }

    /// Return the validated federation descriptor.
    pub fn descriptor(&self) -> &BtcDescriptor<bitcoin::PublicKey> {
        &self.descriptor
    }

    /// Return the parent Bitcoin network.
    pub fn bitcoin_network(&self) -> bitcoin::Network {
        match self.network {
            Network::Liquid => bitcoin::Network::Bitcoin,
            Network::TestnetLiquid => bitcoin::Network::Testnet,
            Network::CustomElements(_) => bitcoin::Network::Regtest,
        }
    }
}

fn classify_fedpeg_program(
    program: &bitcoin::Script,
    script: &bitcoin::Script,
) -> Result<PeginAddressType, Error> {
    let p2wsh_program = bitcoin::ScriptBuf::new_p2wsh(&script.wscript_hash());
    if program == p2wsh_program.as_script() {
        return Ok(PeginAddressType::P2wsh);
    }

    let p2sh_p2wsh_program = bitcoin::ScriptBuf::new_p2sh(&p2wsh_program.script_hash());
    if program == p2sh_p2wsh_program.as_script() {
        return Ok(PeginAddressType::P2shP2wsh);
    }

    Err(Error::InvalidFedPeg(
        "federation program does not commit to the federation script".to_string(),
    ))
}

/// Returns the height of the block containing full federation parameters
///
/// For example in liquid only headers with `(height % 20160) == 0` contains full parameters
#[cfg(not(target_arch = "wasm32"))]
fn height_with_fed_peg_script(network: Network, current_tip: u32) -> u32 {
    // GetValidFedpegScripts # function in elements codebase for valid pegin scripts

    (current_tip / network.dynamic_epoch_length()) * network.dynamic_epoch_length()
}

/// Fetch the fed peg script from the header
pub fn fed_peg_script(header: &BlockHeader) -> Option<bitcoin::ScriptBuf> {
    match &header.ext {
        elements::BlockExtData::Proof { .. } => None,
        elements::BlockExtData::Dynafed { current, .. } => current
            .fedpegscript()
            .map(|e| bitcoin::ScriptBuf::from_bytes(e.clone())),
    }
}

// TODO move this in the trait
/// Fetch the last full header, the full header is the header with the fed peg script which is not always present.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_last_full_header<B: crate::clients::blocking::BlockchainBackend>(
    client: &B,
    network: Network,
    current_tip: u32,
) -> Result<BlockHeader, Error> {
    let height = height_with_fed_peg_script(network, current_tip);
    let mut headers = client.get_headers(&[height], &std::collections::HashMap::new())?;
    headers
        .pop()
        .ok_or(Error::Generic("No headers returned".to_string()))
}

/// Fetch the current federation peg snapshot.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_fed_peg<B: crate::clients::blocking::BlockchainBackend>(
    client: &B,
    network: Network,
    current_tip: u32,
) -> Result<FedPeg, Error> {
    let header = fetch_last_full_header(client, network, current_tip)?;
    FedPeg::from_block_header(network, &header)
}

#[cfg(test)]
mod test {
    use elements::bitcoin;

    use crate::Network;

    use super::{
        classify_fedpeg_program, fed_peg_script, height_with_fed_peg_script, FedPeg,
        PeginAddressType,
    };

    // TODO move in test util
    const FED_PEG_SCRIPT: &str = "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68";

    #[test]
    fn test_height_with_fed_peg_script() {
        assert_eq!(
            height_with_fed_peg_script(Network::Liquid, 2_963_521),
            2_963_520
        );
    }

    #[test]
    fn test_fed_peg_script() {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let script = fed_peg_script(&header).unwrap();
        assert_eq!(script.to_hex_string(), FED_PEG_SCRIPT);
    }

    #[test]
    fn fed_peg_snapshot() {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let fed_peg = FedPeg::from_block_header(Network::Liquid, &header).unwrap();

        assert_eq!(fed_peg.network(), Network::Liquid);
        assert_eq!(fed_peg.script().to_hex_string(), FED_PEG_SCRIPT);
        assert_eq!(fed_peg.address_type(), PeginAddressType::P2wsh);
        assert_eq!(fed_peg.epoch(), 147);
        assert_eq!(fed_peg.epoch_start_height(), 2_963_520);
        assert_eq!(fed_peg.valid_from_height(), 2_963_520);
        assert_eq!(fed_peg.valid_until_height(), 3_003_839);
        assert!(!fed_peg.is_valid_at(2_963_519));
        assert!(fed_peg.is_valid_at(2_963_520));
        assert!(fed_peg.is_valid_at(3_003_839));
        assert!(!fed_peg.is_valid_at(3_003_840));
    }

    #[test]
    fn fed_peg_program_address_types() {
        let script = bitcoin::ScriptBuf::from_hex(FED_PEG_SCRIPT).unwrap();
        let native_program = bitcoin::ScriptBuf::new_p2wsh(&script.wscript_hash());
        assert_eq!(
            classify_fedpeg_program(&native_program, &script).unwrap(),
            PeginAddressType::P2wsh
        );

        let wrapped_program = bitcoin::ScriptBuf::new_p2sh(&native_program.script_hash());
        assert_eq!(
            classify_fedpeg_program(&wrapped_program, &script).unwrap(),
            PeginAddressType::P2shP2wsh
        );
        assert!(classify_fedpeg_program(&bitcoin::ScriptBuf::new(), &script).is_err());
    }

    #[test]
    fn fed_peg_rejects_invalid_epoch_parameters() {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        assert!(FedPeg::from_block_header_with_params(Network::Liquid, &header, 0, 2).is_err());
        assert!(
            FedPeg::from_block_header_with_params(Network::Liquid, &header, 20_160, 0).is_err()
        );
    }
}
