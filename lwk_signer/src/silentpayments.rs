//! BIP-352 signing support for [`SwSigner`].

use elements_miniscript::elements::pset::PartiallySignedTransaction;
use elements_miniscript::elements::schnorr::TweakedPublicKey;
use elements_miniscript::elements::secp256k1_zkp::{
    Keypair, Message, Secp256k1, SecretKey as ZkpSecretKey,
};
use elements_miniscript::elements::sighash::{Prevouts, SighashCache};
use elements_miniscript::elements::{SchnorrSighashType, Script, TxOut};
use lwk_common::get_genesis_hash;
use lwk_common::silentpayments::{
    SilentPaymentAccount, SilentPaymentInputMeta, SilentPaymentPsetMetaError,
    SilentPaymentScanMaterial, SilentPaymentSigner,
};

use crate::software::{SignError, SwSigner};

impl SilentPaymentSigner for SwSigner {
    fn silent_payment_scan_material(
        &self,
        account: SilentPaymentAccount,
    ) -> Result<SilentPaymentScanMaterial, Self::Error> {
        let secp = Secp256k1::new();

        let scan_seckey = self.derive_xprv(&account.scan_path())?.private_key;

        let b_spend = self.derive_xprv(&account.spend_path())?.private_key;
        let spend_pubkey = b_spend.public_key(&secp);

        Ok(SilentPaymentScanMaterial::new(
            account,
            scan_seckey,
            spend_pubkey,
        ))
    }
}

impl SwSigner {
    /// Sign the silent-payment inputs recognized by the ordinary signer entry point.
    pub(crate) fn sign_silent_payment_inputs(
        &self,
        pset: &mut PartiallySignedTransaction,
    ) -> Result<u32, SignError> {
        SilentPaymentPsetSigner::new(self).sign(pset)
    }
}

/// Verifies and signs silent-payment inputs in a PSET.
struct SilentPaymentPsetSigner<'a> {
    signer: &'a SwSigner,
    secp: Secp256k1<elements_miniscript::elements::secp256k1_zkp::All>,
}

impl<'a> SilentPaymentPsetSigner<'a> {
    fn new(signer: &'a SwSigner) -> Self {
        SilentPaymentPsetSigner {
            signer,
            secp: Secp256k1::new(),
        }
    }

    /// Signs verified silent-payment inputs and returns the number signed.
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignError> {
        if !pset.inputs().iter().any(|i| {
            !matches!(
                SilentPaymentInputMeta::read(i),
                Err(SilentPaymentPsetMetaError::Missing)
            )
        }) {
            return Ok(0);
        }

        let prevouts = self.prevouts(pset)?;
        let tx = pset.extract_tx()?;
        let genesis_hash = get_genesis_hash(pset);
        let mut sighash_cache = SighashCache::new(&tx);

        let mut signatures: Vec<Option<(Keypair, Message, SchnorrSighashType)>> = Vec::new();
        for (index, input) in pset.inputs().iter().enumerate() {
            let meta = match SilentPaymentInputMeta::read(input) {
                Ok(meta) => meta,
                Err(SilentPaymentPsetMetaError::Missing) => {
                    signatures.push(None);
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            if input.tap_key_sig.is_some() {
                signatures.push(None);
                continue;
            }

            let keypair = self.verified_keypair(&meta, &prevouts[index])?;
            let hash_ty = input
                .sighash_type
                .and_then(|h| h.schnorr_hash_ty())
                .unwrap_or(SchnorrSighashType::Default);
            let sighash = sighash_cache.taproot_key_spend_signature_hash(
                index,
                &Prevouts::All(&prevouts),
                hash_ty,
                genesis_hash,
            )?;
            let msg = Message::from_digest_slice(sighash.as_ref())?;
            signatures.push(Some((keypair, msg, hash_ty)));
        }

        let mut added = 0;
        for (input, signature) in pset.inputs_mut().iter_mut().zip(signatures) {
            let Some((keypair, msg, hash_ty)) = signature else {
                continue;
            };
            let sig = self.secp.sign_schnorr_no_aux_rand(&msg, &keypair);
            input.tap_key_sig =
                Some(elements_miniscript::elements::schnorr::SchnorrSig { sig, hash_ty });
            added += 1;
        }

        Ok(added)
    }

    /// Returns all witness prevouts required for Taproot sighashing.
    fn prevouts(&self, pset: &PartiallySignedTransaction) -> Result<Vec<TxOut>, SignError> {
        pset.inputs()
            .iter()
            .map(|i| i.witness_utxo.clone().ok_or(SignError::MissingWitnessUtxo))
            .collect()
    }

    /// Verifies metadata and derives the temporary signing key.
    fn verified_keypair(
        &self,
        meta: &SilentPaymentInputMeta,
        prevout: &TxOut,
    ) -> Result<Keypair, SignError> {
        let b_spend = self
            .signer
            .derive_xprv(&meta.account().spend_path())?
            .private_key;
        if b_spend.public_key(&self.secp) != meta.expected_spend_pubkey() {
            return Err(SignError::SilentPaymentSpendPubkeyMismatch);
        }

        let d = b_spend
            .add_tweak(&meta.spend_tweak())
            .map_err(|_| SignError::InvalidTweak)?;
        let d_zkp =
            ZkpSecretKey::from_slice(&d.secret_bytes()).map_err(|_| SignError::InvalidTweak)?;
        let keypair = Keypair::from_secret_key(&self.secp, &d_zkp);

        let (x_only, _parity) = keypair.x_only_public_key();
        let expected_script = Script::new_v1_p2tr_tweaked(TweakedPublicKey::new(x_only));
        if prevout.script_pubkey != expected_script {
            return Err(SignError::SilentPaymentOutputMismatch);
        }

        Ok(keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements_miniscript::bitcoin;
    use elements_miniscript::elements::hashes::Hash;
    use lwk_common::Signer;

    #[test]
    fn scan_material_derivation() {
        let signer = SwSigner::new(lwk_test_util::TEST_MNEMONIC, false).unwrap();
        let secp = Secp256k1::new();

        let account = SilentPaymentAccount::liquid_testnet(0);
        let material = signer.silent_payment_scan_material(account).unwrap();
        let b_scan = signer
            .derive_xprv(&account.scan_path())
            .unwrap()
            .private_key;
        let b_spend = signer
            .derive_xprv(&account.spend_path())
            .unwrap()
            .private_key;
        assert_eq!(material.scan_seckey(), b_scan);
        assert_eq!(material.spend_pubkey(), b_spend.public_key(&secp));

        let disjoint = [
            (
                "accounts differing by index",
                SilentPaymentAccount::liquid_testnet(0),
                SilentPaymentAccount::liquid_testnet(1),
            ),
            (
                "one index across mainnet and testnet",
                SilentPaymentAccount::liquid_mainnet(0),
                SilentPaymentAccount::liquid_testnet(0),
            ),
        ];
        for (why, left, right) in disjoint {
            let a = signer.silent_payment_scan_material(left).unwrap();
            let b = signer.silent_payment_scan_material(right).unwrap();
            assert_ne!(
                a.scan_seckey(),
                b.scan_seckey(),
                "{why} must not share a scan key"
            );
            assert_ne!(
                a.spend_pubkey(),
                b.spend_pubkey(),
                "{why} must not share a spend key"
            );
        }
    }

    /// Builds PSETs containing silent-payment metadata for signer tests.
    struct SpPsetFixture {
        signer: SwSigner,
        account: SilentPaymentAccount,
        tweak: bitcoin::secp256k1::Scalar,
    }

    impl SpPsetFixture {
        fn new() -> Self {
            SpPsetFixture {
                signer: SwSigner::new(lwk_test_util::TEST_MNEMONIC, false).unwrap(),
                account: SilentPaymentAccount::liquid_testnet(0),
                tweak: bitcoin::secp256k1::Scalar::from_be_bytes([0x37; 32]).unwrap(),
            }
        }

        fn spend_pubkey(&self) -> bitcoin::secp256k1::PublicKey {
            self.signer
                .silent_payment_scan_material(self.account)
                .unwrap()
                .spend_pubkey()
        }

        /// The scriptPubKey of the output that `b_spend + tweak` actually controls:
        /// a bare v1 P2TR of `x_only(B_spend + tweak·G)`, no script tree, no BIP-341
        /// taptweak — BIP-352's output convention.
        fn output_script(&self, tweak: &bitcoin::secp256k1::Scalar) -> Script {
            let secp = Secp256k1::new();
            let b_spend = self
                .signer
                .derive_xprv(&self.account.spend_path())
                .unwrap()
                .private_key;
            let d = b_spend.add_tweak(tweak).unwrap();
            let d_zkp = ZkpSecretKey::from_slice(&d.secret_bytes()).unwrap();
            let (x_only, _) = Keypair::from_secret_key(&secp, &d_zkp).x_only_public_key();
            Script::new_v1_p2tr_tweaked(TweakedPublicKey::new(x_only))
        }

        fn txout(&self, script_pubkey: Script) -> TxOut {
            use elements_miniscript::elements::confidential::{Asset, Value};
            use elements_miniscript::elements::AssetId;
            TxOut {
                asset: Asset::Explicit(AssetId::from_slice(&[0x42; 32]).unwrap()),
                value: Value::Explicit(100_000),
                nonce: Default::default(),
                script_pubkey,
                witness: Default::default(),
            }
        }

        /// A PSET with one silent-payment input spending the output the metadata
        /// describes.
        fn pset(&self, meta: SilentPaymentInputMeta) -> PartiallySignedTransaction {
            self.pset_spending(meta, self.output_script(&self.tweak))
        }

        /// As [`Self::pset`], but the coin actually being spent is `spent_script`.
        fn pset_spending(
            &self,
            meta: SilentPaymentInputMeta,
            spent_script: Script,
        ) -> PartiallySignedTransaction {
            use elements_miniscript::elements::pset::{Input, Output, PsbtSighashType};
            use elements_miniscript::elements::{OutPoint, Txid};

            let outpoint = OutPoint::new(Txid::from_slice(&[0x99; 32]).unwrap(), 0);
            let mut input = Input::from_prevout(outpoint);
            input.witness_utxo = Some(self.txout(spent_script));
            input.sighash_type = Some(PsbtSighashType::from_u32(0));
            meta.attach(&mut input);

            let mut pset = PartiallySignedTransaction::new_v2();
            // A real genesis hash, as `TxBuilder` writes (ELIP-101).
            lwk_common::set_genesis_hash(&mut pset, &lwk_common::Network::TestnetLiquid);
            pset.add_input(input);
            pset.add_output(Output::from_txout(self.txout(Script::new())));
            pset
        }

        fn valid_meta(&self) -> SilentPaymentInputMeta {
            self.meta_with(self.account, self.tweak, self.spend_pubkey())
        }

        /// Builds metadata from an account, tweak, and public spend key.
        fn meta_with(
            &self,
            account: SilentPaymentAccount,
            spend_tweak: bitcoin::secp256k1::Scalar,
            spend_pubkey: bitcoin::secp256k1::PublicKey,
        ) -> SilentPaymentInputMeta {
            let dummy_scan = bitcoin::secp256k1::SecretKey::from_slice(&[0x11; 32]).unwrap();
            SilentPaymentScanMaterial::new(account, dummy_scan, spend_pubkey)
                .input_meta(spend_tweak)
        }
    }

    #[test]
    fn honest_metadata_is_signed_exactly_once() {
        let f = SpPsetFixture::new();

        let mut untouched = PartiallySignedTransaction::new_v2();
        assert_eq!(
            f.signer.sign(&mut untouched).unwrap(),
            0,
            "a PSET without silent payment metadata must be left alone"
        );

        let mut pset = f.pset(f.valid_meta());
        assert_eq!(f.signer.sign(&mut pset).unwrap(), 1);
        let first = pset.inputs()[0].tap_key_sig;
        assert!(first.is_some());

        assert_eq!(
            f.signer.sign(&mut pset).unwrap(),
            0,
            "an already-signed input must not be re-signed"
        );
        assert_eq!(pset.inputs()[0].tap_key_sig, first);
    }

    /// Elements Taproot sighashes commit to the chain genesis hash (ELIP-101), which
    /// the signer must read from the PSET rather than assume.
    #[test]
    fn signature_commits_to_the_psets_genesis_hash() {
        use elements_miniscript::elements::sighash::Prevouts;

        let f = SpPsetFixture::new();

        let mut liquid = f.pset(f.valid_meta());
        lwk_common::set_genesis_hash(&mut liquid, &lwk_common::Network::Liquid);
        let mut testnet = f.pset(f.valid_meta());
        lwk_common::set_genesis_hash(&mut testnet, &lwk_common::Network::TestnetLiquid);

        assert_eq!(f.signer.sign(&mut liquid).unwrap(), 1);
        assert_eq!(f.signer.sign(&mut testnet).unwrap(), 1);

        let liquid_sig = liquid.inputs()[0].tap_key_sig.unwrap().sig;
        let testnet_sig = testnet.inputs()[0].tap_key_sig.unwrap().sig;
        assert_ne!(
            liquid_sig, testnet_sig,
            "same transaction on two chains must not produce the same signature; \
             if it does, the genesis hash is not reaching the sighash"
        );

        let secp = Secp256k1::verification_only();
        let output_key = {
            let b_spend = f
                .signer
                .derive_xprv(&f.account.spend_path())
                .unwrap()
                .private_key;
            let d = b_spend.add_tweak(&f.tweak).unwrap();
            let d_zkp = ZkpSecretKey::from_slice(&d.secret_bytes()).unwrap();
            Keypair::from_secret_key(&Secp256k1::new(), &d_zkp)
                .x_only_public_key()
                .0
        };
        let tx = liquid.clone().extract_tx().unwrap();
        let prevouts = [liquid.inputs()[0].witness_utxo.clone().unwrap()];
        let sighash = SighashCache::new(&tx)
            .taproot_key_spend_signature_hash(
                0,
                &Prevouts::All(&prevouts),
                SchnorrSighashType::Default,
                lwk_common::Network::Liquid.genesis_hash(),
            )
            .unwrap();
        let msg = Message::from_digest_slice(sighash.as_ref()).unwrap();
        assert!(
            secp.verify_schnorr(&liquid_sig, &msg, &output_key).is_ok(),
            "signature must verify under the genesis hash the PSET actually carries"
        );
    }

    enum Tampering {
        WrongAccount,
        WrongSpendPubkey,
        WrongTweak,
        ForeignSpentScript,
        MissingWitnessUtxo,
        MalformedMeta,
    }

    impl Tampering {
        fn apply(&self, f: &SpPsetFixture) -> (PartiallySignedTransaction, SignError) {
            match self {
                Tampering::WrongAccount => {
                    let meta = f.meta_with(
                        SilentPaymentAccount::liquid_testnet(7),
                        f.tweak,
                        f.spend_pubkey(),
                    );
                    (f.pset(meta), SignError::SilentPaymentSpendPubkeyMismatch)
                }
                Tampering::WrongSpendPubkey => {
                    let stranger = bitcoin::secp256k1::SecretKey::from_slice(&[0x05; 32])
                        .unwrap()
                        .public_key(&Secp256k1::new());
                    let meta = f.meta_with(f.account, f.tweak, stranger);
                    (f.pset(meta), SignError::SilentPaymentSpendPubkeyMismatch)
                }
                Tampering::WrongTweak => {
                    let meta = f.meta_with(
                        f.account,
                        bitcoin::secp256k1::Scalar::from_be_bytes([0x51; 32]).unwrap(),
                        f.spend_pubkey(),
                    );
                    // The coin spent is still the one the original tweak controls.
                    (
                        f.pset_spending(meta, f.output_script(&f.tweak)),
                        SignError::SilentPaymentOutputMismatch,
                    )
                }
                Tampering::ForeignSpentScript => (
                    f.pset_spending(f.valid_meta(), Script::from(vec![0x00, 0x14, 0xAB])),
                    SignError::SilentPaymentOutputMismatch,
                ),
                Tampering::MissingWitnessUtxo => {
                    let mut pset = f.pset(f.valid_meta());
                    pset.inputs_mut()[0].witness_utxo = None;
                    (pset, SignError::MissingWitnessUtxo)
                }
                Tampering::MalformedMeta => {
                    let mut pset = f.pset(f.valid_meta());
                    let key = pset.inputs()[0]
                        .proprietary
                        .keys()
                        .next()
                        .expect("metadata was attached")
                        .clone();
                    pset.inputs_mut()[0].proprietary.insert(key, vec![0xFF; 5]);
                    (
                        pset,
                        SignError::SilentPaymentMeta(
                            lwk_common::silentpayments::SilentPaymentPsetMetaError::Malformed,
                        ),
                    )
                }
            }
        }
    }

    #[test]
    fn tampered_metadata_is_refused_and_left_unsigned() {
        let f = SpPsetFixture::new();
        let cases = [
            Tampering::WrongAccount,
            Tampering::WrongSpendPubkey,
            Tampering::WrongTweak,
            Tampering::ForeignSpentScript,
            Tampering::MissingWitnessUtxo,
            Tampering::MalformedMeta,
        ];

        for case in &cases {
            let (mut pset, expected) = case.apply(&f);
            let err = f
                .signer
                .sign(&mut pset)
                .expect_err("tampered metadata must not be signed");

            // By variant: SignError is not PartialEq, and the exact payload of the
            // metadata error is pinned by its own tests in lwk_common.
            assert_eq!(
                std::mem::discriminant(&err),
                std::mem::discriminant(&expected),
                "expected {expected:?}, got {err:?}"
            );
            assert!(
                pset.inputs()[0].tap_key_sig.is_none(),
                "a refused input must be left unsigned"
            );
        }
    }
}
