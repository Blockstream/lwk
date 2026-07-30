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

/// A snapshot of federation pegin parameters and their guaranteed validity window.
///
/// A `FedPeg` is created from a full dynafed block header. It retains the exact
/// federation script and program used to derive pegin addresses, so callers do
/// not need to pass an unversioned federation descriptor around. The validity
/// window is conservative: the same parameters may remain valid in later
/// epochs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FedPeg {
    network: Network,
    script: bitcoin::ScriptBuf,
    program: bitcoin::ScriptBuf,
    descriptor: BtcDescriptor<bitcoin::PublicKey>,
    epoch: u32,
    epoch_start_height: u32,
    valid_from_height: u32,
    guaranteed_valid_until_height: u32,
    address_type: PeginAddressType,
}

impl FedPeg {
    /// Create a federation peg snapshot from a full dynafed block header.
    ///
    /// The guaranteed validity window uses the epoch parameters defined by
    /// `network`.
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
        let guaranteed_valid_until_height = epoch
            .checked_add(total_valid_epochs)
            .and_then(|epoch| epoch.checked_mul(epoch_length))
            .and_then(|height| height.checked_sub(1))
            .ok_or_else(|| Error::InvalidFedPeg("validity horizon overflow".to_string()))?;

        Ok(Self {
            network,
            script,
            program,
            descriptor,
            epoch,
            epoch_start_height,
            valid_from_height: header.height,
            guaranteed_valid_until_height,
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

    /// Return the last block height through which this snapshot is guaranteed valid.
    ///
    /// The federation may retain the same pegin parameters in later epochs, so
    /// a greater height does not prove that the snapshot has expired.
    pub fn guaranteed_valid_until_height(&self) -> u32 {
        self.guaranteed_valid_until_height
    }

    /// Return the inclusive guaranteed Liquid block-height validity window.
    ///
    /// The federation may retain the same pegin parameters after this window.
    pub fn guaranteed_validity_window(&self) -> std::ops::RangeInclusive<u32> {
        self.valid_from_height..=self.guaranteed_valid_until_height
    }

    /// Return whether this snapshot is guaranteed valid at `height`.
    ///
    /// A false result does not prove that the snapshot has expired; determining
    /// that requires comparison with the federation parameters valid at
    /// `height`.
    pub fn is_guaranteed_valid_at(&self, height: u32) -> bool {
        self.guaranteed_validity_window().contains(&height)
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

/// A Bitcoin pegin address together with all data needed to identify its claim.
///
/// The embedded [`FedPeg`] preserves the exact federation parameters and
/// guaranteed validity window used to derive the address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeginAddress {
    address: bitcoin::Address,
    claim_script: elements::Script,
    derivation_index: u32,
    fed_peg: FedPeg,
}

impl PeginAddress {
    pub(crate) fn new(
        address: bitcoin::Address,
        claim_script: elements::Script,
        derivation_index: u32,
        fed_peg: FedPeg,
    ) -> Self {
        Self {
            address,
            claim_script,
            derivation_index,
            fed_peg,
        }
    }

    /// Return the Bitcoin deposit address.
    pub fn address(&self) -> &bitcoin::Address {
        &self.address
    }

    /// Return the Liquid claim script committed to by the deposit address.
    pub fn claim_script(&self) -> &elements::Script {
        &self.claim_script
    }

    /// Return the wallet derivation index used for the claim script.
    pub fn derivation_index(&self) -> u32 {
        self.derivation_index
    }

    /// Return the federation snapshot used to derive this address.
    pub fn fed_peg(&self) -> &FedPeg {
        &self.fed_peg
    }

    /// Return the Bitcoin address type.
    pub fn address_type(&self) -> PeginAddressType {
        self.fed_peg.address_type()
    }

    /// Return the inclusive guaranteed Liquid block-height validity window.
    ///
    /// The federation may retain the same pegin parameters after this window.
    pub fn guaranteed_validity_window(&self) -> std::ops::RangeInclusive<u32> {
        self.fed_peg.guaranteed_validity_window()
    }

    /// Return whether this address is guaranteed claimable at the given Liquid height.
    ///
    /// A false result does not prove that the address has expired.
    pub fn is_guaranteed_valid_at(&self, height: u32) -> bool {
        self.fed_peg.is_guaranteed_valid_at(height)
    }
}

/// A Bitcoin transaction output paying a [`PeginAddress`].
///
/// Construction finds the output paying the exact Bitcoin script pubkey
/// derived for the pegin and rejects transactions with zero or multiple
/// matching outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeginDeposit {
    pegin_address: PeginAddress,
    transaction: bitcoin::Transaction,
    outpoint: bitcoin::OutPoint,
    output: bitcoin::TxOut,
}

impl PeginDeposit {
    /// Create and validate a pegin deposit.
    pub fn new(
        pegin_address: PeginAddress,
        transaction: bitcoin::Transaction,
    ) -> Result<Self, Error> {
        let expected_script = pegin_address.address().script_pubkey();
        let mut matching_outputs = transaction
            .output
            .iter()
            .enumerate()
            .filter(|(_, output)| output.script_pubkey == expected_script);
        let (vout, output) = matching_outputs.next().ok_or(Error::PeginOutputNotFound)?;
        if matching_outputs.next().is_some() {
            return Err(Error::PeginOutputAmbiguous);
        }
        let output = output.clone();
        let vout = u32::try_from(vout).map_err(|_| Error::PeginOutputIndexOverflow { vout })?;

        let outpoint = bitcoin::OutPoint::new(transaction.compute_txid(), vout);
        Ok(Self {
            pegin_address,
            transaction,
            outpoint,
            output,
        })
    }

    /// Return the pegin address paid by this deposit.
    pub fn pegin_address(&self) -> &PeginAddress {
        &self.pegin_address
    }

    /// Return the Bitcoin transaction containing this deposit.
    pub fn transaction(&self) -> &bitcoin::Transaction {
        &self.transaction
    }

    /// Return the Bitcoin outpoint identifying this deposit.
    pub fn outpoint(&self) -> bitcoin::OutPoint {
        self.outpoint
    }

    /// Return the Bitcoin transaction output containing the deposited amount.
    pub fn output(&self) -> &bitcoin::TxOut {
        &self.output
    }

    /// Return the deposited amount.
    pub fn amount(&self) -> bitcoin::Amount {
        self.output.value
    }
}

/// A pegin deposit authenticated by a Bitcoin transaction inclusion proof.
///
/// Construction verifies the partial Merkle tree against its block header and
/// requires the deposit transaction among the matched transactions. It does
/// not establish that the header belongs to the best Bitcoin chain or that the
/// deposit has enough confirmations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeginFunding {
    deposit: PeginDeposit,
    txout_proof: bitcoin::MerkleBlock,
    referenced_block: bitcoin::BlockHash,
}

impl PeginFunding {
    /// Create and validate proven pegin funding.
    pub fn new(deposit: PeginDeposit, txout_proof: bitcoin::MerkleBlock) -> Result<Self, Error> {
        let mut matched_txids = Vec::new();
        let mut matched_indexes = Vec::new();
        txout_proof
            .extract_matches(&mut matched_txids, &mut matched_indexes)
            .map_err(|e| Error::InvalidPeginProof(e.to_string()))?;

        let txid = deposit.outpoint().txid;
        if !matched_txids.contains(&txid) {
            return Err(Error::PeginTransactionNotInProof { txid });
        }

        let referenced_block = txout_proof.header.block_hash();
        Ok(Self {
            deposit,
            txout_proof,
            referenced_block,
        })
    }

    /// Return the authenticated pegin deposit.
    pub fn deposit(&self) -> &PeginDeposit {
        &self.deposit
    }

    /// Return the Bitcoin transaction inclusion proof.
    pub fn txout_proof(&self) -> &bitcoin::MerkleBlock {
        &self.txout_proof
    }

    /// Return the Bitcoin block header hash committed to by the proof.
    pub fn referenced_block(&self) -> bitcoin::BlockHash {
        self.referenced_block
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
    use elements::bitcoin::hashes::Hash;

    use crate::{Error, Network};

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
        assert_eq!(fed_peg.guaranteed_valid_until_height(), 3_003_839);
        assert!(!fed_peg.is_guaranteed_valid_at(2_963_519));
        assert!(fed_peg.is_guaranteed_valid_at(2_963_520));
        assert!(fed_peg.is_guaranteed_valid_at(3_003_839));
        assert!(!fed_peg.is_guaranteed_valid_at(3_003_840));
    }

    fn test_pegin_address() -> super::PeginAddress {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let fed_peg = FedPeg::from_block_header(Network::TestnetLiquid, &header).unwrap();
        let descriptor: crate::WolletDescriptor = lwk_test_util::PEGIN_TEST_DESC.parse().unwrap();
        descriptor.pegin_address(0, &fed_peg).unwrap()
    }

    fn test_deposit_transaction(pegin_address: &super::PeginAddress) -> bitcoin::Transaction {
        bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(1),
                    script_pubkey: bitcoin::ScriptBuf::new(),
                },
                bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(100_000),
                    script_pubkey: pegin_address.address().script_pubkey(),
                },
            ],
        }
    }

    fn test_txout_proof(txid: bitcoin::Txid) -> bitcoin::MerkleBlock {
        let header = bitcoin::block::Header {
            version: bitcoin::block::Version::ONE,
            prev_blockhash: bitcoin::BlockHash::all_zeros(),
            merkle_root: bitcoin::TxMerkleNode::from_raw_hash(txid.to_raw_hash()),
            time: 0,
            bits: bitcoin::CompactTarget::from_consensus(0),
            nonce: 0,
        };
        bitcoin::MerkleBlock::from_header_txids_with_predicate(&header, &[txid], |_| true)
    }

    #[test]
    fn pegin_deposit_validates_output() {
        let pegin_address = test_pegin_address();
        let transaction = test_deposit_transaction(&pegin_address);
        let txid = transaction.compute_txid();
        let deposit = super::PeginDeposit::new(pegin_address.clone(), transaction.clone()).unwrap();

        assert_eq!(deposit.pegin_address(), &pegin_address);
        assert_eq!(deposit.transaction(), &transaction);
        assert_eq!(deposit.outpoint(), bitcoin::OutPoint::new(txid, 1));
        assert_eq!(deposit.output(), &transaction.output[1]);
        assert_eq!(deposit.amount(), bitcoin::Amount::from_sat(100_000));
    }

    #[test]
    fn pegin_deposit_rejects_missing_output() {
        let pegin_address = test_pegin_address();
        let mut transaction = test_deposit_transaction(&pegin_address);
        transaction.output.pop();

        assert!(matches!(
            super::PeginDeposit::new(pegin_address, transaction),
            Err(Error::PeginOutputNotFound)
        ));
    }

    #[test]
    fn pegin_deposit_rejects_ambiguous_output() {
        let pegin_address = test_pegin_address();
        let mut transaction = test_deposit_transaction(&pegin_address);
        transaction.output.push(transaction.output[1].clone());

        assert!(matches!(
            super::PeginDeposit::new(pegin_address, transaction),
            Err(Error::PeginOutputAmbiguous)
        ));
    }

    #[test]
    fn pegin_funding_validates_txout_proof() {
        let pegin_address = test_pegin_address();
        let transaction = test_deposit_transaction(&pegin_address);
        let txid = transaction.compute_txid();
        let deposit = super::PeginDeposit::new(pegin_address, transaction).unwrap();
        let proof = test_txout_proof(txid);
        let referenced_block = proof.header.block_hash();
        let funding = super::PeginFunding::new(deposit.clone(), proof.clone()).unwrap();

        assert_eq!(funding.deposit(), &deposit);
        assert_eq!(funding.txout_proof(), &proof);
        assert_eq!(funding.referenced_block(), referenced_block);
    }

    #[test]
    fn pegin_funding_rejects_invalid_txout_proof() {
        let pegin_address = test_pegin_address();
        let transaction = test_deposit_transaction(&pegin_address);
        let txid = transaction.compute_txid();
        let deposit = super::PeginDeposit::new(pegin_address, transaction).unwrap();
        let mut proof = test_txout_proof(txid);
        proof.header.merkle_root = bitcoin::TxMerkleNode::all_zeros();

        assert!(matches!(
            super::PeginFunding::new(deposit, proof),
            Err(Error::InvalidPeginProof(_))
        ));
    }

    #[test]
    fn pegin_funding_rejects_unmatched_transaction() {
        let pegin_address = test_pegin_address();
        let transaction = test_deposit_transaction(&pegin_address);
        let txid = transaction.compute_txid();
        let deposit = super::PeginDeposit::new(pegin_address, transaction).unwrap();
        let mut other_transaction = test_deposit_transaction(deposit.pegin_address());
        other_transaction.output[0].value = bitcoin::Amount::from_sat(2);
        let other_txid = other_transaction.compute_txid();
        let proof = test_txout_proof(other_txid);

        assert!(matches!(
            super::PeginFunding::new(deposit, proof),
            Err(Error::PeginTransactionNotInProof {
                txid: unmatched_txid
            }) if unmatched_txid == txid
        ));
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
    fn p2sh_wrapped_pegin_address() {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let mut fed_peg = FedPeg::from_block_header(Network::TestnetLiquid, &header).unwrap();
        let native_program = bitcoin::ScriptBuf::new_p2wsh(&fed_peg.script.wscript_hash());
        fed_peg.program = bitcoin::ScriptBuf::new_p2sh(&native_program.script_hash());
        fed_peg.address_type = PeginAddressType::P2shP2wsh;

        let descriptor: crate::WolletDescriptor = lwk_test_util::PEGIN_TEST_DESC.parse().unwrap();
        let address = descriptor.pegin_address(0, &fed_peg).unwrap();
        assert_eq!(address.address_type(), PeginAddressType::P2shP2wsh);
        assert_eq!(
            address.address().address_type(),
            Some(bitcoin::AddressType::P2sh)
        );
    }

    #[test]
    fn pegin_address_rejects_nested_claim_script() {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let fed_peg = FedPeg::from_block_header(Network::TestnetLiquid, &header).unwrap();
        let nested = format!(
            "{})",
            lwk_test_util::PEGIN_TEST_DESC.replace("elwpkh(", "elsh(wpkh(")
        );
        let descriptor: crate::WolletDescriptor = nested.parse().unwrap();

        assert!(matches!(
            descriptor.pegin_address(0, &fed_peg),
            Err(crate::Error::UnsupportedPeginClaimScript)
        ));
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
