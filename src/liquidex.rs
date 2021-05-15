use std::collections::HashSet;
use std::io::Write;

use aes_gcm_siv::aead::{generic_array::GenericArray, AeadInPlace, NewAead};
use aes_gcm_siv::Aes256GcmSiv;

use rand::Rng;

use elements::bitcoin::hashes::{sha256, sha256d, Hash};
use elements::confidential::{Asset, Nonce, Value};
use elements::encode::Encodable;
use elements::slip77::MasterBlindingKey;

use secp256k1_zkp::{All, Secp256k1};

use crate::error::Error;
use crate::model::Unblinded;
use crate::utils::derive_blinder;

fn _liquidex_derive_blinder(
    master_blinding_key: &MasterBlindingKey,
    previous_outpoint: &elements::OutPoint,
    is_asset_blinder: bool,
) -> Result<secp256k1_zkp::SecretKey, secp256k1_zkp::UpstreamError> {
    // LiquiDEX proposals do not know in advance all inputs of
    // final transaction, compute the hash only from the previous
    // outpoint we know is being spent.
    let hash_prevout = {
        let mut enc = sha256d::Hash::engine();
        previous_outpoint.consensus_encode(&mut enc).unwrap();
        sha256d::Hash::from_engine(enc)
    };

    // LiquiDEX proposals output vout is choosen by the taker,
    // for the blinder computation use a vout that may not
    // occur in a transaction.
    derive_blinder(
        master_blinding_key,
        &hash_prevout,
        u32::MAX,
        is_asset_blinder,
    )
}

fn _liquidex_aes_key(
    master_blinding_key: &MasterBlindingKey,
    script: &elements::Script,
) -> Result<[u8; 32], Error> {
    // TODO: consider using tagged hashes
    const TAG: &[u8; 16] = b"liquidex_aes_key";
    let mut engine = sha256::Hash::engine();
    engine.write(TAG)?;
    engine.write(&master_blinding_key.0[..])?;
    engine.write(&script.as_bytes())?;
    Ok(sha256::Hash::from_engine(engine).into_inner())
}

fn _liquidex_aes_nonce(
    master_blinding_key: &MasterBlindingKey,
    previous_outpoint: &elements::OutPoint,
    asset: &elements::confidential::Asset,
    value: &elements::confidential::Value,
    script: &elements::Script,
) -> Result<[u8; 12], Error> {
    match (asset, value) {
        (Asset::Confidential(_, _), Value::Confidential(_, _)) => {}
        _ => {
            return Err(Error::Generic(
                "Asset and Value must be confidential".to_string(),
            ));
        }
    }
    // TODO: consider using tagged hashes
    const TAG: &[u8; 18] = b"liquidex_aes_nonce";
    let mut engine = sha256::Hash::engine();
    engine.write(TAG)?;
    engine.write(&master_blinding_key.0[..])?;
    previous_outpoint.consensus_encode(&mut engine)?;
    engine.write(&asset.commitment().unwrap())?;
    engine.write(&value.commitment().unwrap())?;
    engine.write(&script.as_bytes())?;
    let mut out = [0u8; 12];
    out.copy_from_slice(&sha256::Hash::from_engine(engine).into_inner()[..12]);
    Ok(out)
}

/// Blind a LiquiDEX maker transaction.
/// The maker has no control on the rangeproof, thus it can't rely on it to recover the unblinding
/// data. Use deterministic blinders and use the nonce field to encrypt the output value.
pub fn _liquidex_blind(
    master_blinding_key: &MasterBlindingKey,
    tx: &mut elements::Transaction,
    secp: &Secp256k1<All>,
) -> Result<(), Error> {
    if tx.input.len() != 1 || tx.output.len() != 1 {
        return Err(Error::Generic(
            "Unexpected LiquiDEX maker transaction".to_string(),
        ));
    }
    let (asset, value) = match (tx.output[0].asset, tx.output[0].value, tx.output[0].nonce) {
        (Asset::Explicit(asset), Value::Explicit(value), Nonce::Null) => (asset, value),
        _ => {
            return Err(Error::Generic(
                "Unexpected LiquiDEX maker transaction".to_string(),
            ));
        }
    };

    let asset_blinder =
        _liquidex_derive_blinder(master_blinding_key, &tx.input[0].previous_output, true)?;
    let value_blinder =
        _liquidex_derive_blinder(master_blinding_key, &tx.input[0].previous_output, false)?;

    let asset_tag = secp256k1_zkp::Tag::from(asset.into_inner().into_inner());
    let asset_generator = secp256k1_zkp::Generator::new_blinded(secp, asset_tag, asset_blinder);
    let value_commitment =
        secp256k1_zkp::PedersenCommitment::new(secp, value, value_blinder, asset_generator);

    tx.output[0].asset = Asset::from_commitment(&asset_generator.serialize())?;
    tx.output[0].value = Value::from_commitment(&value_commitment.serialize())?;

    let key = _liquidex_aes_key(master_blinding_key, &tx.output[0].script_pubkey)?;
    let key = GenericArray::from_slice(&key);
    let cipher = Aes256GcmSiv::new(&key);

    let aes_nonce = _liquidex_aes_nonce(
        master_blinding_key,
        &tx.input[0].previous_output,
        &tx.output[0].asset,
        &tx.output[0].value,
        &tx.output[0].script_pubkey,
    )?;
    let aes_nonce = GenericArray::from_slice(&aes_nonce);

    let mut rng = rand::thread_rng();
    let nonce_commitment = loop {
        // On average does 2 loops.
        let mut text = [0u8; 16];
        text[..8].copy_from_slice(&value.to_le_bytes());
        rng.fill(&mut text[8..]);
        let mut text = text.to_vec();
        cipher.encrypt_in_place(aes_nonce, b"", &mut text)?;
        let mut candidate = [0u8; 33];
        candidate[0] = 0x02;
        candidate[1..].copy_from_slice(&text);
        if let Ok(pk) = secp256k1_zkp::PublicKey::from_slice(&candidate) {
            break pk.serialize();
        }
    };

    tx.output[0].nonce = elements::confidential::Nonce::from_commitment(&nonce_commitment)?;

    Ok(())
}

pub fn _liquidex_unblind(
    master_blinding_key: &MasterBlindingKey,
    tx: &elements::Transaction,
    vout: u32,
    secp: &Secp256k1<All>,
    assets: &HashSet<elements::issuance::AssetId>,
) -> Result<Unblinded, Error> {
    // check vout is reasonable
    let vout = vout as usize;
    if vout > tx.output.len() || vout > tx.input.len() {
        return Err(Error::Generic("LiquiDEX error".to_string()));
    }
    // check output is blinded
    match (
        tx.output[vout].asset,
        tx.output[vout].value,
        tx.output[vout].nonce,
    ) {
        (Asset::Confidential(_, _), Value::Confidential(_, _), Nonce::Confidential(_, _)) => {}
        _ => {
            return Err(Error::Generic("LiquiDEX error".to_string()));
        }
    }
    // FIXME: check input has sighash single | anyonecanpay
    // FIXME: check input has a script belonging to the wallet
    // compute blinders
    let asset_blinder =
        _liquidex_derive_blinder(master_blinding_key, &tx.input[vout].previous_output, true)?;
    let value_blinder =
        _liquidex_derive_blinder(master_blinding_key, &tx.input[vout].previous_output, false)?;

    // compute key
    let key = _liquidex_aes_key(master_blinding_key, &tx.output[vout].script_pubkey)?;
    let key = GenericArray::from_slice(&key);
    let cipher = Aes256GcmSiv::new(&key);

    // compute aes nonce
    let aes_nonce = _liquidex_aes_nonce(
        master_blinding_key,
        &tx.input[vout].previous_output,
        &tx.output[vout].asset,
        &tx.output[vout].value,
        &tx.output[vout].script_pubkey,
    )?;
    let aes_nonce = GenericArray::from_slice(&aes_nonce);

    // parse nonce_commitment
    let nonce_commitment = tx.output[vout].nonce.commitment().unwrap();
    let mut text = vec![];
    text.extend(&nonce_commitment[1..]);

    // decrypt value
    cipher.decrypt_in_place(aes_nonce, b"", &mut text)?;
    let mut value_bytes = [0u8; 8];
    value_bytes.copy_from_slice(&text[..8]);
    let value = u64::from_le_bytes(value_bytes);

    // check value matches value commitment
    let tx_asset_generator =
        secp256k1_zkp::Generator::from_slice(&tx.output[vout].asset.commitment().unwrap())?;
    let tx_value_commitment = secp256k1_zkp::PedersenCommitment::from_slice(
        &tx.output[vout].value.commitment().unwrap(),
    )?;
    let value_commitment =
        secp256k1_zkp::PedersenCommitment::new(secp, value, value_blinder, tx_asset_generator);
    if value_commitment != tx_value_commitment {
        return Err(Error::Generic("LiquiDEX error".to_string()));
    }

    let mut asset: Option<elements::issuance::AssetId> = None;
    // loop on assets
    for candidate in assets {
        // check asset matches asset commitment
        let asset_tag = secp256k1_zkp::Tag::from(candidate.into_inner().into_inner());
        let asset_generator = secp256k1_zkp::Generator::new_blinded(secp, asset_tag, asset_blinder);
        if asset_generator == tx_asset_generator {
            asset = Some(candidate.clone());
            break;
        }
    }

    // check a match happened
    if asset.is_none() {
        return Err(Error::Generic("LiquiDEX error".to_string()));
    }
    let asset = asset.unwrap();

    // return unblinded
    Ok(Unblinded {
        asset,
        asset_blinder,
        value_blinder,
        value,
    })
}

#[cfg(test)]
mod tests {
    use crate::liquidex::{_liquidex_blind, _liquidex_unblind};
    use crate::transaction::add_input;

    #[test]
    fn test_liquidex_roundtrip() {
        assert_eq!(2, 2);
        let seed = [0u8; 32];
        let master_blinding_key = elements::slip77::MasterBlindingKey::new(&seed);
        let mut tx = elements::Transaction {
            version: 2,
            lock_time: 0,
            input: vec![],
            output: vec![],
        };
        // add input
        let outpoint = elements::OutPoint::new(tx.txid(), 0);
        add_input(&mut tx, outpoint);
        // add output
        let asset = [1u8; 32];
        let asset = elements::issuance::AssetId::from_slice(&asset).unwrap();
        let value = 10;
        let script = elements::Script::from(vec![0x51]);
        let new_out = elements::TxOut {
            asset: elements::confidential::Asset::Explicit(asset),
            value: elements::confidential::Value::Explicit(value),
            nonce: elements::confidential::Nonce::Null,
            script_pubkey: script,
            witness: elements::TxOutWitness::default(),
        };
        tx.output.push(new_out);

        let secp = secp256k1_zkp::Secp256k1::new();
        _liquidex_blind(&master_blinding_key, &mut tx, &secp).unwrap();
        let mut assets = std::collections::HashSet::<elements::issuance::AssetId>::new();
        assets.insert(asset.clone());
        let unblinded = _liquidex_unblind(&master_blinding_key, &tx, 0, &secp, &assets).unwrap();
        assert_eq!(unblinded.asset, asset);
        assert_eq!(unblinded.value, value);
    }
}
