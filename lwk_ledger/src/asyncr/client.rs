use core::fmt::Debug;
use std::str::FromStr;

use crate::{
    apdu::{APDUCmdVec, StatusWord},
    command,
    error::LiquidClientError,
    interpreter::{get_merkleized_map_commitment, ClientCommandInterpreter},
    parse_multisig,
    psbt::*,
    wallet::WalletPolicy,
    AddressType, Version, WalletPubKey,
};
use elements_miniscript::elements::{
    pset::PartiallySignedTransaction as Psbt, Address, AddressParams,
};
use elements_miniscript::slip77::MasterBlindingKey;
use elements_miniscript::{
    bitcoin::bip32::ChildNumber,
    elements::bitcoin::{
        bip32::{DerivationPath, Fingerprint, Xpub},
        consensus::encode::deserialize_partial,
        secp256k1::ecdsa,
        VarInt,
    },
};

#[derive(Debug)]
pub struct LiquidClient<T: Transport> {
    transport: T,
}

impl<T: Transport> LiquidClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    async fn make_request(
        &self,
        req: &APDUCmdVec,
        interpreter: Option<&mut ClientCommandInterpreter>,
    ) -> Result<Vec<u8>, LiquidClientError<T::Error>> {
        let (mut sw, mut data) = self
            .transport
            .exchange(req)
            .await
            .map_err(LiquidClientError::Transport)?;

        if let Some(interpreter) = interpreter {
            while sw == StatusWord::InterruptedExecution {
                let response = interpreter.execute(data)?;
                let res = self
                    .transport
                    .exchange(&command::continue_interrupted(response))
                    .await
                    .map_err(LiquidClientError::Transport)?;
                sw = res.0;
                data = res.1;
            }
        }

        if sw != StatusWord::OK {
            Err(LiquidClientError::Device {
                status: sw,
                command: req.ins,
            })
        } else {
            Ok(data)
        }
    }

    /// Returns the currently running app's name, version and state flags
    pub async fn get_version(
        &self,
    ) -> Result<(String, String, Vec<u8>), LiquidClientError<T::Error>> {
        let cmd = command::get_version();
        let data = self.make_request(&cmd, None).await?;
        if data.is_empty() || data[0] != 0x01 {
            return Err(LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data,
            });
        }

        let (name, i): (String, usize) =
            deserialize_partial(&data[1..]).map_err(|_| LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data: data.clone(),
            })?;

        let (version, j): (String, usize) = deserialize_partial(&data[i + 1..]).map_err(|_| {
            LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data: data.clone(),
            }
        })?;

        let (flags, _): (Vec<u8>, usize) =
            deserialize_partial(&data[i + j + 1..]).map_err(|_| {
                LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data: data.clone(),
                }
            })?;

        Ok((name, version, flags))
    }

    /// Retrieve the master fingerprint.
    pub async fn get_master_fingerprint(&self) -> Result<Fingerprint, LiquidClientError<T::Error>> {
        let cmd = command::get_master_fingerprint();
        self.make_request(&cmd, None).await.and_then(|data| {
            if data.len() < 4 {
                Err(LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data,
                })
            } else {
                let mut fg = [0x00; 4];
                fg.copy_from_slice(&data[0..4]);
                Ok(Fingerprint::from(fg))
            }
        })
    }

    /// Retrieve the bip32 extended pubkey derived with the given path
    /// and optionally display it on screen
    pub async fn get_extended_pubkey(
        &self,
        path: &DerivationPath,
        display: bool,
    ) -> Result<Xpub, LiquidClientError<T::Error>> {
        let cmd = command::get_extended_pubkey(path, display);
        self.make_request(&cmd, None).await.and_then(|data| {
            Xpub::from_str(&String::from_utf8_lossy(&data)).map_err(|_| {
                LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data,
                }
            })
        })
    }

    /// Registers the given wallet policy, returns the wallet ID and HMAC.
    #[allow(clippy::type_complexity)]
    pub async fn register_wallet(
        &self,
        wallet: &WalletPolicy,
    ) -> Result<([u8; 32], [u8; 32]), LiquidClientError<T::Error>> {
        let cmd = command::register_wallet(wallet);
        let mut intpr = ClientCommandInterpreter::new();
        intpr.add_known_preimage(wallet.serialize());
        let keys: Vec<String> = wallet.keys.iter().map(|k| k.to_string()).collect();
        intpr.add_known_list(&keys);
        // necessary for version 1 of the protocol (introduced in version 2.1.0)
        intpr.add_known_preimage(wallet.descriptor_template.as_bytes().to_vec());
        let (id, hmac) = self
            .make_request(&cmd, Some(&mut intpr))
            .await
            .and_then(|data| {
                if data.len() < 64 {
                    Err(LiquidClientError::UnexpectedResult {
                        command: cmd.ins,
                        data,
                    })
                } else {
                    let mut id = [0x00; 32];
                    id.copy_from_slice(&data[0..32]);
                    let mut hmac = [0x00; 32];
                    hmac.copy_from_slice(&data[32..64]);
                    Ok((id, hmac))
                }
            })?;

        /*
        #[cfg(feature = "paranoid_client")]
        {
            let device_addr = self.get_wallet_address(wallet, Some(&hmac), false, 0, false)?;
            self.check_address(wallet, false, 0, &device_addr)?;
        }
         * */

        Ok((id, hmac))
    }

    /// For a given wallet that was already registered on the device (or a standard wallet that does not need registration),
    /// returns the address for a certain `change`/`address_index` combination.
    pub async fn get_wallet_address(
        &self,
        wallet: &WalletPolicy,
        wallet_hmac: Option<&[u8; 32]>,
        change: bool,
        address_index: u32,
        display: bool,
        // TODO: move to self?
        params: &'static AddressParams,
    ) -> Result<Address, LiquidClientError<T::Error>> {
        let mut intpr = ClientCommandInterpreter::new();
        intpr.add_known_preimage(wallet.serialize());
        let keys: Vec<String> = wallet.keys.iter().map(|k| k.to_string()).collect();
        intpr.add_known_list(&keys);
        // necessary for version 1 of the protocol (introduced in version 2.1.0)
        intpr.add_known_preimage(wallet.descriptor_template.as_bytes().to_vec());
        let cmd = command::get_wallet_address(wallet, wallet_hmac, change, address_index, display);
        let address = self
            .make_request(&cmd, Some(&mut intpr))
            .await
            .and_then(|data| {
                Address::parse_with_params(&String::from_utf8_lossy(&data), params).map_err(|_| {
                    LiquidClientError::UnexpectedResult {
                        command: cmd.ins,
                        data,
                    }
                })
            })?;

        /*
        #[cfg(feature = "paranoid_client")]
        {
            self.check_address(wallet, change, address_index, &address)?;
        }
         * */

        Ok(address)
    }

    /// Signs a PSBT using a registered wallet (or a standard wallet that does not need registration).
    /// Signature requires explicit approval from the user.
    #[allow(clippy::type_complexity)]
    pub async fn sign_psbt(
        &self,
        psbt: &Psbt,
        wallet: &WalletPolicy,
        wallet_hmac: Option<&[u8; 32]>,
    ) -> Result<Vec<(usize, PartialSignature)>, LiquidClientError<T::Error>> {
        let mut intpr = ClientCommandInterpreter::new();
        intpr.add_known_preimage(wallet.serialize());
        let keys: Vec<String> = wallet.keys.iter().map(|k| k.to_string()).collect();
        intpr.add_known_list(&keys);
        // necessary for version 1 of the protocol (introduced in version 2.1.0)
        intpr.add_known_preimage(wallet.descriptor_template.as_bytes().to_vec());

        let global_map: Vec<(Vec<u8>, Vec<u8>)> = get_v2_global_pairs(psbt)
            .into_iter()
            .map(deserialize_pair)
            .collect();
        intpr.add_known_mapping(&global_map);
        let global_mapping_commitment = get_merkleized_map_commitment(&global_map);

        // TODO: consider removing sig_script and witness
        let unsigned_tx = psbt
            .extract_tx()
            .map_err(|_| LiquidClientError::InvalidPsbt)?;
        let mut input_commitments: Vec<Vec<u8>> = Vec::with_capacity(psbt.inputs().len());
        for (index, input) in psbt.inputs().iter().enumerate() {
            let txin = unsigned_tx
                .input
                .get(index)
                .ok_or(LiquidClientError::InvalidPsbt)?;
            let input_map: Vec<(Vec<u8>, Vec<u8>)> = get_v2_input_pairs(input, txin)
                .into_iter()
                .map(deserialize_pair)
                .collect();
            intpr.add_known_mapping(&input_map);
            input_commitments.push(get_merkleized_map_commitment(&input_map));
        }
        let input_commitments_root = intpr.add_known_list(&input_commitments);

        let mut output_commitments: Vec<Vec<u8>> = Vec::with_capacity(psbt.outputs().len());
        for (index, output) in psbt.outputs().iter().enumerate() {
            let txout = unsigned_tx
                .output
                .get(index)
                .ok_or(LiquidClientError::InvalidPsbt)?;
            let output_map: Vec<(Vec<u8>, Vec<u8>)> = get_v2_output_pairs(output, txout)
                .into_iter()
                .map(deserialize_pair)
                .collect();
            intpr.add_known_mapping(&output_map);
            output_commitments.push(get_merkleized_map_commitment(&output_map));
        }
        let output_commitments_root = intpr.add_known_list(&output_commitments);

        let cmd = command::sign_psbt(
            &global_mapping_commitment,
            psbt.n_inputs(),
            &input_commitments_root,
            psbt.n_outputs(),
            &output_commitments_root,
            wallet,
            wallet_hmac,
        );

        self.make_request(&cmd, Some(&mut intpr)).await?;

        let results = intpr.yielded();
        if results.iter().any(|res| res.len() <= 1) {
            return Err(LiquidClientError::UnexpectedResult {
                command: cmd.ins,
                data: results.into_iter().fold(Vec::new(), |mut acc, res| {
                    acc.extend(res);
                    acc
                }),
            });
        }

        let mut signatures = Vec::new();
        for result in results {
            let (input_index, i): (VarInt, usize) =
                deserialize_partial(&result).map_err(|_| LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data: result.clone(),
                })?;

            signatures.push((
                input_index.0 as usize,
                PartialSignature::from_slice(&result[i..]).map_err(|_| {
                    LiquidClientError::UnexpectedResult {
                        command: cmd.ins,
                        data: result.clone(),
                    }
                })?,
            ));
        }

        Ok(signatures)
    }

    /// Sign a message with the key derived with the given derivation path.
    /// Result is the header byte (31-34: P2PKH compressed) and the ecdsa signature.
    pub async fn sign_message(
        &self,
        message: &[u8],
        path: &DerivationPath,
    ) -> Result<(u8, ecdsa::Signature), LiquidClientError<T::Error>> {
        let chunks: Vec<&[u8]> = message.chunks(64).collect();
        let mut intpr = ClientCommandInterpreter::new();
        let message_commitment_root = intpr.add_known_list(&chunks);
        let cmd = command::sign_message(message.len(), &message_commitment_root, path);
        self.make_request(&cmd, Some(&mut intpr))
            .await
            .and_then(|data| {
                Ok((
                    data[0],
                    ecdsa::Signature::from_compact(&data[1..]).map_err(|_| {
                        LiquidClientError::UnexpectedResult {
                            command: cmd.ins,
                            data: data.to_vec(),
                        }
                    })?,
                ))
            })
    }

    pub async fn sign(
        &self,
        pset: &mut Psbt,
    ) -> std::result::Result<u32, LiquidClientError<T::Error>> {
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
        let master_fp = self.get_master_fingerprint().await?;

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
                        let xpub = self.get_extended_pubkey(&path, false).await?;
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
                        .expect("FIXME");
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
                let (_id, hmac) = self.register_wallet(wallet_policy).await?;
                Some(hmac)
            } else {
                None
            };
            let partial_sigs = self.sign_psbt(pset, wallet_policy, hmac.as_ref()).await?;
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

    /// Retrieve the SLIP77 master blinding key.
    pub async fn get_master_blinding_key(
        &self,
    ) -> Result<MasterBlindingKey, LiquidClientError<T::Error>> {
        let cmd = command::get_master_blinding_key();
        self.make_request(&cmd, None).await.and_then(|data| {
            if data.len() != 32 {
                Err(LiquidClientError::UnexpectedResult {
                    command: cmd.ins,
                    data,
                })
            } else {
                let mut fg = [0x00; 32];
                fg.copy_from_slice(&data[0..32]);
                Ok(MasterBlindingKey::from(fg))
            }
        })
    }
}

/// Asynchronous communication layer between the bitcoin client and the Ledger device.
pub trait Transport {
    type Error: Debug;
    fn exchange(
        &self,
        command: &APDUCmdVec,
    ) -> impl std::future::Future<Output = Result<(StatusWord, Vec<u8>), Self::Error>>; // TODO use async in trait instead of returning Future once supported by rust
}
