use elements::{TxInWitness, TxOutWitness};

pub const DUST_VALUE: u64 = 546;

pub fn strip_witness(tx: &mut elements::Transaction) {
    for input in tx.input.iter_mut() {
        input.witness = TxInWitness::default();
    }
    for output in tx.output.iter_mut() {
        output.witness = TxOutWitness::default();
    }
}
