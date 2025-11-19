mod apdu;
mod client;
pub mod command;
mod error;
mod interpreter;
mod merkle;
mod psbt;
mod transport_tcp;
mod wallet;

#[cfg(feature = "serial")]
mod transport_hid;

#[cfg(feature = "test_emulator")]
mod ledger_emulator;

#[cfg(feature = "asyncr")]
pub mod asyncr;

use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
#[cfg(feature = "test_emulator")]
pub use ledger_emulator::TestLedgerEmulator;

pub use ledger_apdu::APDUAnswer;

// Adapted from
// https://github.com/LedgerHQ/app-bitcoin-new/tree/master/bitcoin_client_rs
pub use client::LiquidClient;
use client::Transport;
pub use psbt::PartialSignature;
pub use transport_tcp::TransportTcp;
pub use wallet::{AddressType, Version, WalletPolicy, WalletPubKey};

pub use apdu::{APDUCmdVec, StatusWord};

use elements_miniscript::confidential::slip77;
use elements_miniscript::elements::bitcoin::bip32::{
    ChildNumber, DerivationPath, Fingerprint, Xpub,
};
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use elements_miniscript::elements::{
    bitcoin::key::PublicKey,
    bitcoin::sign_message::MessageSignature,
    opcodes::{
        all::{OP_CHECKMULTISIG, OP_PUSHNUM_1, OP_PUSHNUM_16},
        All,
    },
    script::Instruction,
    Script,
};

use lwk_common::Signer;

#[derive(Debug)]
pub struct Ledger<T: Transport> {
    /// Ledger Liquid Client
    pub client: LiquidClient<T>,
}

impl Ledger<TransportTcp> {
    pub fn new(port: u16) -> Self {
        let client = LiquidClient::new(TransportTcp::new(port).expect("TODO"));
        Self { client }
    }
}

#[cfg(feature = "serial")]
impl Ledger<transport_hid::TransportHID> {
    pub fn new_hid() -> Self {
        let h = ledger_transport_hid::hidapi::HidApi::new().expect("unable to get HIDAPI");
        let hid = ledger_transport_hid::TransportNativeHID::new(&h).unwrap();
        let client = LiquidClient::new(transport_hid::TransportHID::new(hid));
        Self { client }
    }
}

pub type Error = error::LiquidClientError<TransportTcp>;

impl<T: Transport> Signer for &Ledger<T> {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> std::result::Result<u32, Self::Error> {
        // Set the default values some fields that Ledger requires
        if pset.global.tx_data.fallback_locktime.is_none() {
            pset.global.tx_data.fallback_locktime =
                Some(elements_miniscript::elements::LockTime::ZERO);
        }
        for input in pset.inputs_mut() {
            if input.sequence.is_none() {
                input.sequence = Some(elements_miniscript::elements::Sequence::default());
            }
        }

        // Use a map to avoid inserting a wallet twice
        let mut wallets = std::collections::HashMap::<String, WalletPolicy>::new();
        let mut n_sigs = 0;
        let master_fp = self.fingerprint()?;

        // Figure out which wallets are signing
        'outer: for input in pset.inputs() {
            let is_p2wpkh = input
                .witness_utxo
                .as_ref()
                .map(|u| u.script_pubkey.is_v0_p2wpkh())
                .unwrap_or(false);
            let is_p2sh = input
                .witness_utxo
                .as_ref()
                .map(|u| u.script_pubkey.is_p2sh())
                .unwrap_or(false);
            let is_p2shwpkh = is_p2sh
                && input
                    .redeem_script
                    .as_ref()
                    .map(|x| x.is_v0_p2wpkh())
                    .unwrap_or(false);
            // Singlesig
            if is_p2wpkh || is_p2shwpkh {
                // We expect exactly one element
                if let Some((fp, path)) = input.bip32_derivation.values().next() {
                    if fp == &master_fp {
                        // TODO: check path
                        // path has len 3
                        // path has all hardened
                        // path has purpose matching address type
                        // path has correct coin type
                        let mut v: Vec<ChildNumber> = path.clone().into();
                        v.truncate(3);
                        let path: DerivationPath = v.into();

                        // Do we care about the descriptor blinding key here?
                        let name = "".to_string();
                        let version = Version::V2;
                        // TODO: cache xpubs
                        let xpub = self
                            .client
                            .get_extended_pubkey(&path, false)
                            .map_err(to_dbg)?;
                        let key = WalletPubKey::from(((*fp, path.clone()), xpub));
                        let keys = vec![key];
                        let desc = if is_p2wpkh {
                            "wpkh(@0/**)"
                        } else {
                            "sh(wpkh(@0/**))"
                        };
                        let wallet_policy =
                            WalletPolicy::new(name, version, desc.to_string(), keys);
                        let is_change = false;
                        if let Ok(d) = wallet_policy.get_descriptor(is_change) {
                            wallets.insert(d, wallet_policy);
                        }
                    }
                }
            } else {
                let is_p2wsh = input
                    .witness_utxo
                    .as_ref()
                    .map(|u| u.script_pubkey.is_v0_p2wsh())
                    .unwrap_or(false);
                let details = input.witness_script.as_ref().and_then(parse_multisig);
                // Multisig
                if is_p2wsh {
                    if let Some((threshold, pubkeys)) = details {
                        let mut keys: Vec<WalletPubKey> = vec![];
                        for pubkey in pubkeys {
                            if let Some((fp, path)) = input.bip32_derivation.get(&pubkey) {
                                let mut v: Vec<ChildNumber> = path.clone().into();
                                v.truncate(3);
                                let path: DerivationPath = v.into();
                                let keysource = (*fp, path);
                                if let Some(xpub) = pset.global.xpub.iter().find_map(|(x, ks)| {
                                    if ks == &keysource {
                                        Some(x)
                                    } else {
                                        None
                                    }
                                }) {
                                    let mut key = WalletPubKey::from((keysource, *xpub));
                                    key.multipath = Some("/**".to_string());
                                    keys.push(key);
                                } else {
                                    // Global xpub not available, cannot reconstruct the script
                                    continue 'outer;
                                }
                            } else {
                                // No keysource for pubkey in script
                                // Either the script is not ours or data is missing
                                continue 'outer;
                            }
                        }
                        let sorted = false;
                        let wallet_policy = WalletPolicy::new_multisig(
                            "todo".to_string(),
                            Version::V1,
                            AddressType::NativeSegwit,
                            threshold as usize,
                            keys,
                            sorted,
                            None,
                        )
                        .map_err(to_dbg)?;
                        let is_change = false;
                        if let Ok(d) = wallet_policy.get_descriptor(is_change) {
                            wallets.insert(d, wallet_policy);
                        }
                    }
                }
            }
        }

        // For each wallet, sign
        for wallet_policy in wallets.values() {
            let hmac = if wallet_policy.threshold.is_some() {
                // Register multisig wallets
                let (_id, hmac) = self.client.register_wallet(wallet_policy).map_err(to_dbg)?;
                Some(hmac)
            } else {
                None
            };
            let partial_sigs = self
                .client
                .sign_psbt(pset, wallet_policy, hmac.as_ref())
                .map_err(to_dbg)?;
            n_sigs += partial_sigs.len();

            // Add sigs to pset
            for (input_idx, sig) in partial_sigs {
                let input = &mut pset.inputs_mut()[input_idx];
                for (public_key, (fp, _origin)) in &input.bip32_derivation {
                    if fp == &master_fp {
                        // TODO: user the pubkey from PartialSignature to insert in partial_sigs
                        let sig_vec = match sig {
                            PartialSignature::Sig(_, sig) => sig.to_vec(),
                            _ => panic!("FIXME: support taproot sig or raise error"),
                        };
                        input.partial_sigs.insert(*public_key, sig_vec);
                        // FIXME: handle cases where we have multiple pubkeys with master fingerprint
                        break;
                    }
                }
            }
        }

        Ok(n_sigs as u32)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Self::Error> {
        self.client.get_extended_pubkey(path, false).map_err(to_dbg)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Self::Error> {
        self.client.get_master_blinding_key().map_err(to_dbg)
    }

    fn fingerprint(&self) -> std::result::Result<Fingerprint, Self::Error> {
        self.client.get_master_fingerprint().map_err(to_dbg)
    }

    fn sign_message(
        &self,
        _message: &str,
        _path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        todo!(); // TODO: use internal methods to implement
    }
}

fn to_dbg(e: impl std::fmt::Debug) -> Error {
    Error::ClientError(format!("{e:?}"))
}

impl<T: Transport> Signer for Ledger<T> {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> std::result::Result<u32, Self::Error> {
        Signer::sign(&self, pset)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }

    fn fingerprint(&self) -> std::result::Result<Fingerprint, Self::Error> {
        Signer::fingerprint(&self)
    }

    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        Signer::sign_message(&self, message, path)
    }
}

// "duplicated" from Jade
// taken and adapted from:
// https://github.com/rust-bitcoin/rust-bitcoin/blob/37daf4620c71dc9332c3e08885cf9de696204bca/bitcoin/src/blockdata/script/borrowed.rs#L266
#[allow(unused)]
pub fn parse_multisig(script: &Script) -> Option<(u32, Vec<PublicKey>)> {
    fn decode_pushnum(op: All) -> Option<u8> {
        let start: u8 = OP_PUSHNUM_1.into_u8();
        let end: u8 = OP_PUSHNUM_16.into_u8();
        if start < op.into_u8() && end >= op.into_u8() {
            Some(op.into_u8() - start + 1)
        } else {
            None
        }
    }

    let required_sigs;

    let mut instructions = script.instructions();
    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if let Some(pushnum) = decode_pushnum(op) {
            required_sigs = pushnum;
        } else {
            return None;
        }
    } else {
        return None;
    }

    let mut num_pubkeys: u8 = 0;
    let mut pubkeys = vec![];
    while let Some(Ok(instruction)) = instructions.next() {
        match instruction {
            Instruction::PushBytes(pubkey) => {
                let pk = PublicKey::from_slice(pubkey).expect("FIXME");
                pubkeys.push(pk);
                num_pubkeys += 1;
            }
            Instruction::Op(op) => {
                if let Some(pushnum) = decode_pushnum(op) {
                    if pushnum != num_pubkeys {
                        return None;
                    }
                }
                break;
            }
        }
    }

    if required_sigs > num_pubkeys {
        return None;
    }

    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if op != OP_CHECKMULTISIG {
            return None;
        }
    } else {
        return None;
    }

    if instructions.next().is_none() {
        Some((required_sigs.into(), pubkeys))
    } else {
        None
    }
}

const LEDGER_CHANNEL: u16 = 0x0101;

pub fn read_multi_apdu(apdu_answers: Vec<Vec<u8>>) -> Result<Vec<u8>, Error> {
    let mut result = vec![];
    let mut expected_apdu_len = 0usize;

    for (sequence_idx, el) in apdu_answers.into_iter().enumerate() {
        let res = el.len();

        if (sequence_idx == 0 && res < 7) || res < 5 {
            return Err(Error::ClientError(
                "Read error. Incomplete header".to_string(),
            ));
        }

        let mut rdr = Cursor::new(&el);

        let rcv_channel = rdr
            .read_u16::<BigEndian>()
            .map_err(|_| Error::ClientError("Invalid channel".to_string()))?;
        let rcv_tag = rdr
            .read_u8()
            .map_err(|_| Error::ClientError("Invalid tag".to_string()))?;
        let rcv_seq_idx = rdr
            .read_u16::<BigEndian>()
            .map_err(|_| Error::ClientError("Invalid sequence idx".to_string()))?;

        if rcv_channel != LEDGER_CHANNEL {
            return Err(Error::ClientError("Invalid channel".to_string()));
        }
        if rcv_tag != 0x05u8 {
            return Err(Error::ClientError("Invalid tag".to_string()));
        }

        if rcv_seq_idx != sequence_idx as u16 {
            return Err(Error::ClientError("Invalid sequence idx".to_string()));
        }

        if rcv_seq_idx == 0 {
            expected_apdu_len = rdr
                .read_u16::<BigEndian>()
                .map_err(|_| Error::ClientError("Invalid expected apdu len".to_string()))?
                as usize;
        }

        let needs_more = expected_apdu_len > (result.len() + el.len()); // TODO check off by one

        let start = rdr.position() as usize;
        let end = if needs_more {
            el.len()
        } else {
            expected_apdu_len - result.len() + start
        };

        let new_chunk = &el[start..end];

        result.extend_from_slice(new_chunk);

        if result.len() >= expected_apdu_len {
            return Ok(result);
        }
    }
    Err(Error::ClientError("Incomplete APDU".to_string()))
}

const LEDGER_PACKET_WRITE_SIZE: u8 = 64;

// based on https://github.com/Zondax/ledger-rs/blob/master/ledger-transport-hid/src/lib.rs
// with the notable difference we don't use the prefix 0x00
pub fn write_apdu(apdu_command: &APDUCmdVec) -> Vec<[u8; LEDGER_PACKET_WRITE_SIZE as usize]> {
    let channel = LEDGER_CHANNEL;
    let apdu_command = apdu_command.serialize();
    let mut results = vec![];
    let command_length = apdu_command.len();
    let mut in_data = Vec::with_capacity(command_length + 2);
    in_data.push(((command_length >> 8) & 0xFF) as u8);
    in_data.push((command_length & 0xFF) as u8);
    in_data.extend_from_slice(&apdu_command);

    let mut buffer = vec![0u8; LEDGER_PACKET_WRITE_SIZE as usize];
    buffer[0] = ((channel >> 8) & 0xFF) as u8; // channel big endian
    buffer[1] = (channel & 0xFF) as u8; // channel big endian
    buffer[2] = 0x05u8;

    for (sequence_idx, chunk) in in_data
        .chunks((LEDGER_PACKET_WRITE_SIZE - 5) as usize)
        .enumerate()
    {
        buffer[3] = ((sequence_idx >> 8) & 0xFF) as u8; // sequence_idx big endian
        buffer[4] = (sequence_idx & 0xFF) as u8; // sequence_idx big endian
        buffer[5..5 + chunk.len()].copy_from_slice(chunk);

        results.push(buffer.clone().try_into().unwrap());
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_apdu() {
        // messages taken from xpub requests in the browser
        let message1 = [
            1, 1, 5, 0, 0, 0, 113, 116, 112, 117, 98, 68, 67, 119, 89, 106, 112, 68, 104, 85, 100,
            80, 71, 80, 53, 114, 83, 51, 119, 103, 78, 103, 49, 51, 109, 84, 114, 114, 106, 66,
            117, 71, 56, 86, 57, 86, 112, 87, 98, 121, 112, 116, 88, 54, 84, 82, 80, 98, 78, 111,
            90, 86, 88, 115,
        ]
        .to_vec();
        let message2 = [
            1, 1, 5, 0, 1, 111, 86, 85, 83, 107, 67, 106, 109, 81, 56, 106, 74, 121, 99, 106, 117,
            68, 75, 66, 98, 57, 101, 97, 116, 97, 83, 121, 109, 88, 97, 107, 84, 84, 97, 71, 105,
            102, 120, 82, 54, 107, 109, 86, 115, 102, 70, 101, 104, 72, 49, 90, 103, 74, 84, 144,
            0, 0, 0, 0,
        ]
        .to_vec();
        let result = read_multi_apdu(vec![message1, message2]).unwrap();
        assert_eq!(
            result,
            vec![
                116, 112, 117, 98, 68, 67, 119, 89, 106, 112, 68, 104, 85, 100, 80, 71, 80, 53,
                114, 83, 51, 119, 103, 78, 103, 49, 51, 109, 84, 114, 114, 106, 66, 117, 71, 56,
                86, 57, 86, 112, 87, 98, 121, 112, 116, 88, 54, 84, 82, 80, 98, 78, 111, 90, 86,
                88, 115, 111, 86, 85, 83, 107, 67, 106, 109, 81, 56, 106, 74, 121, 99, 106, 117,
                68, 75, 66, 98, 57, 101, 97, 116, 97, 83, 121, 109, 88, 97, 107, 84, 84, 97, 71,
                105, 102, 120, 82, 54, 107, 109, 86, 115, 102, 70, 101, 104, 72, 49, 90, 103, 74,
                84, 144, 0
            ]
        );
        let answer = APDUAnswer::from_answer(result).unwrap();
        let status = StatusWord::try_from(answer.retcode()).unwrap_or(StatusWord::Unknown);
        // let vec = answer.data().to_vec();
        assert_eq!(status, StatusWord::OK);
    }

    #[test]
    fn test_read_apdu_single() {
        let get_version_test_vector_array = [
            1, 14, 76, 105, 113, 117, 105, 100, 32, 82, 101, 103, 116, 101, 115, 116, 5, 50, 46,
            50, 46, 51, 1, 2, 144, 0,
        ];

        let received_apdu_ledger = [
            1, 1, 5, 0, 0, 0, 26, 1, 14, 76, 105, 113, 117, 105, 100, 32, 82, 101, 103, 116, 101,
            115, 116, 5, 50, 46, 50, 46, 51, 1, 2, 144, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let result = read_multi_apdu(vec![received_apdu_ledger.to_vec()]).unwrap();
        assert_eq!(result, get_version_test_vector_array.to_vec());
    }

    #[test]
    fn test_fix_return_error() {
        let d = [[
            1, 1, 5, 0, 0, 0, 67, 66, 246, 203, 175, 41, 59, 76, 239, 143, 8, 205, 206, 188, 195,
            151, 107, 194, 119, 208, 91, 66, 183, 226, 62, 33, 83, 168, 81, 140, 125, 100, 200,
            140, 146, 242, 46, 52, 250, 248, 37, 179, 244, 196, 225, 203, 90, 152, 201, 177, 38,
            128, 184, 233, 230, 21, 233, 229,
        ]
        .to_vec()]
        .to_vec();

        let result = read_multi_apdu(d).unwrap_err();
        assert_eq!(result.to_string(), "Client Error: Incomplete APDU");
    }

    #[test]
    fn test_write_apdu() {
        let command = crate::command::get_version();
        assert_eq!(&command.serialize(), &[176u8, 1, 0, 0, 0]);
        let results = write_apdu(&command);
        assert_eq!(
            results,
            vec![[
                1, 1, 5, 0, 0, 0, 5, 176, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0
            ]]
        );
    }
}
