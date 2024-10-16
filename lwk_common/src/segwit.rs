use elements_miniscript::elements::Script;

/// Determine if a transaction input is provably segwit
pub fn is_provably_segwit(prevout_scriptpubkey: &Script, redeem_script: &Option<Script>) -> bool {
    if prevout_scriptpubkey.is_witness_program() {
        // Native Segwit
        return true;
    }
    if prevout_scriptpubkey.is_p2sh() {
        if let Some(redeem_script) = redeem_script {
            if redeem_script.is_witness_program()
                && &redeem_script.to_p2sh() == prevout_scriptpubkey
            {
                // Segwit Wrapped
                return true;
            }
        }
    }
    false
}
