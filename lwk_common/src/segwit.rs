use elements_miniscript::elements::Script;

/// Whether a script pubkey is provably segwit
///
/// The redeem script is necessary for P2SH wrapped scripts.
pub fn is_provably_segwit(scriptpubkey: &Script, redeem_script: &Option<Script>) -> bool {
    if scriptpubkey.is_witness_program() {
        // Native Segwit
        return true;
    }
    if scriptpubkey.is_p2sh() {
        if let Some(redeem_script) = redeem_script {
            if redeem_script.is_witness_program() && &redeem_script.to_p2sh() == scriptpubkey {
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
    use elements_miniscript::elements::schnorr::UntweakedPublicKey;
    use elements_miniscript::elements::secp256k1_zkp::Secp256k1;
    use elements_miniscript::elements::{PubkeyHash, WPubkeyHash};
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
        let wsh = multi.wscript_hash();
        let p2sh_multi = multi.to_p2sh();
        assert!(!is_provably_segwit(&p2sh_multi, &Some(multi)));

        let s = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let wpkh = WPubkeyHash::from_str(s).unwrap();
        let p2wpkh = Script::new_v0_wpkh(&wpkh);
        assert!(is_provably_segwit(&p2wpkh, &None));

        let p2wsh_multi = Script::new_v0_wsh(&wsh);
        assert!(is_provably_segwit(&p2wsh_multi, &None));

        let s = "0202020202020202020202020202020202020202020202020202020202020202";
        let upk = UntweakedPublicKey::from_str(s).unwrap();
        let secp = Secp256k1::new();
        let p2tr = Script::new_v1_p2tr(&secp, upk, None);
        assert!(is_provably_segwit(&p2tr, &None));

        let p2sh_p2wpkh = p2wpkh.to_p2sh();
        assert!(is_provably_segwit(&p2sh_p2wpkh, &Some(p2wpkh)));
        assert!(!is_provably_segwit(&p2sh_p2wpkh, &None));
        assert!(!is_provably_segwit(&p2sh_p2wpkh, &Some(Script::default())));

        let p2sh_p2wsh = p2wsh_multi.to_p2sh();
        assert!(is_provably_segwit(&p2sh_p2wsh, &Some(p2wsh_multi)));
        assert!(!is_provably_segwit(&p2sh_p2wsh, &None));
        assert!(!is_provably_segwit(&p2sh_p2wsh, &Some(Script::default())));
    }
}
