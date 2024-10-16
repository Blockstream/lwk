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

#[cfg(test)]
mod test {
    use super::*;
    use elements_miniscript::elements::bitcoin::PublicKey;
    use elements_miniscript::elements::PubkeyHash;
    use std::str::FromStr;

    #[test]
    fn test_provably_segwit() {
        let op_return = Script::new_op_return(&[]);
        assert!(!is_provably_segwit(&op_return, &None));

        let s = "020202020202020202020202020202020202020202020202020202020202020202";
        let pk = PublicKey::from_str(s).unwrap();
        let p2pk = Script::new_p2pk(&pk);
        assert!(!is_provably_segwit(&p2pk, &None));

        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let pkh = PubkeyHash::from_str(s).unwrap();
        let p2pkh = Script::new_p2pkh(&pkh);
        assert!(!is_provably_segwit(&p2pkh, &None));

        let s = "522102ebc62c20f1e09e169a88745f60f6dac878c92db5c7ed78c6703d2d0426a01f942102c2d59d677122bc292048833003fd5cb19d27d32896b1d0feec654c291f7ede9e52ae";
        let multi = Script::from_str(s).unwrap();
        let p2sh_multi = multi.to_p2sh();
        assert!(!is_provably_segwit(&p2sh_multi, &Some(multi)));
    }
}
