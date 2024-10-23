/// APDU commands  for the Bitcoin application.
///
use elements_miniscript::elements::bitcoin::{
    bip32::{ChildNumber, DerivationPath},
    consensus::encode::{self, VarInt},
};

use crate::apdu::{apdu, apdu_empty, APDUCmdVec, Cla, LiquidCommandCode};

use super::wallet::WalletPolicy;

/// Creates the APDU Command to retrieve the app's name, version and state flags.
pub fn get_version() -> APDUCmdVec {
    let mut apdu = apdu_empty(
        Cla::Default,
        LiquidCommandCode::GetVersionOrContinueInterrupted,
    );
    apdu.p2 = 0x00;
    apdu
}

/// Creates the APDU Command to retrieve the master fingerprint.
pub fn get_master_fingerprint() -> APDUCmdVec {
    apdu_empty(Cla::Bitcoin, LiquidCommandCode::GetMasterFingerprint)
}

/// Creates the APDU command required to get the extended pubkey with the given derivation path.
pub fn get_extended_pubkey(path: &DerivationPath, display: bool) -> APDUCmdVec {
    let child_numbers: &[ChildNumber] = path.as_ref();
    let data: Vec<u8> = child_numbers.iter().fold(
        vec![
            if display { 1_u8 } else { b'\0' },
            child_numbers.len() as u8,
        ],
        |mut acc, &x| {
            acc.extend_from_slice(&u32::from(x).to_be_bytes());
            acc
        },
    );

    apdu(Cla::Bitcoin, LiquidCommandCode::GetExtendedPubkey, data)
}

/// Creates the APDU command required to register the given wallet policy.
pub fn register_wallet(policy: &WalletPolicy) -> APDUCmdVec {
    let bytes = policy.serialize();
    let mut data = encode::serialize(&VarInt(bytes.len() as u64));
    data.extend(bytes);

    apdu(Cla::Bitcoin, LiquidCommandCode::RegisterWallet, data)
}

/// Creates the APDU command required to retrieve an address for the given wallet.
pub fn get_wallet_address(
    policy: &WalletPolicy,
    hmac: Option<&[u8; 32]>,
    change: bool,
    address_index: u32,
    display: bool,
) -> APDUCmdVec {
    let mut data: Vec<u8> = Vec::with_capacity(70);
    data.push(if display { 1_u8 } else { b'\0' });
    data.extend_from_slice(&policy.id());
    data.extend_from_slice(hmac.unwrap_or(&[b'\0'; 32]));
    data.push(if change { 1_u8 } else { b'\0' });
    data.extend_from_slice(&address_index.to_be_bytes());

    apdu(Cla::Bitcoin, LiquidCommandCode::GetWalletAddress, data)
}

/// Creates the APDU command required to sign a psbt.
pub fn sign_psbt(
    global_mapping_commitment: &[u8],
    inputs_number: usize,
    input_commitments_root: &[u8; 32],
    outputs_number: usize,
    output_commitments_root: &[u8; 32],
    policy: &WalletPolicy,
    hmac: Option<&[u8; 32]>,
) -> APDUCmdVec {
    let mut data: Vec<u8> = Vec::new();
    data.extend_from_slice(global_mapping_commitment);
    data.extend(encode::serialize(&VarInt(inputs_number as u64)));
    data.extend_from_slice(input_commitments_root);
    data.extend(encode::serialize(&VarInt(outputs_number as u64)));
    data.extend_from_slice(output_commitments_root);
    data.extend_from_slice(&policy.id());
    data.extend_from_slice(hmac.unwrap_or(&[b'\0'; 32]));

    apdu(Cla::Bitcoin, LiquidCommandCode::SignPSBT, data)
}

/// Creates the APDU Command to sign a message.
pub fn sign_message(
    message_length: usize,
    message_commitment_root: &[u8; 32],
    path: &DerivationPath,
) -> APDUCmdVec {
    let child_numbers: &[ChildNumber] = path.as_ref();
    let mut data: Vec<u8> =
        child_numbers
            .iter()
            .fold(vec![child_numbers.len() as u8], |mut acc, &x| {
                acc.extend_from_slice(&u32::from(x).to_be_bytes());
                acc
            });
    data.extend(encode::serialize(&VarInt(message_length as u64)));
    data.extend_from_slice(message_commitment_root);

    apdu(Cla::Bitcoin, LiquidCommandCode::SignMessage, data)
}

/// Creates the APDU Command to retrieve the master blinding key.
pub fn get_master_blinding_key() -> APDUCmdVec {
    apdu_empty(Cla::Bitcoin, LiquidCommandCode::LiquidGetMasterBlindingKey)
}

/// Creates the APDU command to CONTINUE.
pub fn continue_interrupted(data: Vec<u8>) -> APDUCmdVec {
    apdu(
        Cla::Framework,
        LiquidCommandCode::GetVersionOrContinueInterrupted,
        data,
    )
}
