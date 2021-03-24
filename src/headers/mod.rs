use crate::error::Error;
use ::bitcoin::hashes::{sha256d, Hash};
use ::bitcoin::{TxMerkleNode, Txid};
use electrum_client::GetMerkleRes;
use std::io::Write;

pub mod liquid;

/// compute the merkle root from the merkle path of a tx in electrum format (note the hash.reverse())
fn compute_merkle_root(txid: &Txid, merkle: GetMerkleRes) -> Result<TxMerkleNode, Error> {
    let mut pos = merkle.pos;
    let mut current = txid.into_inner();

    for mut hash in merkle.merkle {
        let mut engine = sha256d::Hash::engine();
        hash.reverse();
        if pos % 2 == 0 {
            engine.write(&current)?;
            engine.write(&hash)?;
        } else {
            engine.write(&hash)?;
            engine.write(&current)?;
        }
        current = sha256d::Hash::from_engine(engine).into_inner();
        pos /= 2;
    }

    Ok(TxMerkleNode::from_slice(&current)?)
}
