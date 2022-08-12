pub fn tx_to_hex(tx: &elements::Transaction) -> String {
    hex::encode(elements::encode::serialize(tx))
}
