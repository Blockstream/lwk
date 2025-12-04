use elements_miniscript::elements::{self, BlockHeader};

use elements::bitcoin::bip32::Xpriv;
use elements::bitcoin::Network;
use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use elements::encode::Decodable;
use elements::hex::{FromHex, ToHex};
use elements::pset::PartiallySignedTransaction;
use elements::{AssetId, TxOutWitness, Txid};
use elements::{Block, TxOutSecrets};
use elements_miniscript::descriptor::checksum::desc_checksum;
use pulldown_cmark::{CodeBlockKind, Event, Tag};
use rand::{thread_rng, Rng};
use std::str::FromStr;

mod registry;
mod test_env;
mod waterfalls;
pub use test_env::{TestEnv, TestEnvBuilder};

const DEFAULT_FEE_RATE: f32 = 100.0;

pub const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
pub const TEST_MNEMONIC_XPUB: &str =
"tpubD6NzVbkrYhZ4XYa9MoLt4BiMZ4gkt2faZ4BcmKu2a9te4LDpQmvEz2L2yDERivHxFPnxXXhqDRkUNnQCpZggCyEZLBktV7VaSmwayqMJy1s";
pub const TEST_MNEMONIC_SLIP77: &str =
    "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

pub const DEFAULT_SPECULOS_MNEMONIC: &str = "glory promote mansion idle axis finger extra february uncover one trip resource lawn turtle enact monster seven myth punch hobby comfort wild raise skin";

// Constants for pegins
// test vector created with:
// ```
// $ elements-cli getnetworkinfo | jq .version
// 230201
// $ elements-cli getblockchaininfo | jq .blocks
// 2976078
// elements-cli getsidechaininfo | jq '.current_fedpegscripts[0]'`
// "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68"
// $ elements-cli getpeginaddress
// {
// "mainchain_address": "bc1qyya0twwz58kgfslpdgsygeq0r4nngl9tkt89v6phk8nqrwyenwrq5h0dk8",
// "claim_script": "0014a15906e643f2c9635527ab8658d370e8eaf149b5"
// }
// ```
pub const FED_PEG_SCRIPT: &str = "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68";
pub const FED_PEG_SCRIPT_ASM: &str = "OP_PUSHNUM_11 OP_PUSHBYTES_33 020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b67817261 OP_PUSHBYTES_33 02675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af99 OP_PUSHBYTES_33 02896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d48 OP_PUSHBYTES_33 029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c OP_PUSHBYTES_33 02a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc4010 OP_PUSHBYTES_33 031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb OP_PUSHBYTES_33 03079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b OP_PUSHBYTES_33 03111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2 OP_PUSHBYTES_33 0318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa0840174 OP_PUSHBYTES_33 03230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de1 OP_PUSHBYTES_33 035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a6 OP_PUSHBYTES_33 03bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c OP_PUSHBYTES_33 03cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d17546 OP_PUSHBYTES_33 03d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d424828 OP_PUSHBYTES_33 03ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a OP_PUSHNUM_15 OP_CHECKMULTISIG OP_IFDUP OP_NOTIF OP_PUSHBYTES_2 c00f OP_CSV OP_VERIFY OP_PUSHNUM_2 OP_PUSHBYTES_33 03aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79 OP_PUSHBYTES_33 0291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807 OP_PUSHBYTES_33 0386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb OP_PUSHNUM_3 OP_CHECKMULTISIG OP_ENDIF";
pub const FED_PEG_DESC: &str = "wsh(or_d(multi(11,020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b67817261,02675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af99,02896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d48,029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c,02a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc4010,031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb,03079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b,03111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2,0318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa0840174,03230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de1,035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a6,03bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c,03cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d17546,03d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d424828,03ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a),and_v(v:older(4032),multi(2,03aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79,0291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807,0386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb))))#7jwwklk4";

pub const PEGIN_TEST_DESC: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))";
pub const PEGIN_TEST_ADDR: &str = "tb1qqkq6czql4zqwsylgrfzttjrn5wjeqmwfq5yn80p39amxtnkng9lsyjwm6v"; // tweak_index = 0

/// Descriptor with 11 txs on testnet
pub const TEST_DESCRIPTOR: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

pub fn liquid_block_1() -> Block {
    let raw = include_bytes!(
        "../test_data/afafbbdfc52a45e51a3b634f391f952f6bdfd14ef74b34925954b4e20d0ad639.raw"
    );
    Block::consensus_decode(&raw[..]).unwrap()
}

pub fn liquid_block_header_2_963_520() -> BlockHeader {
    let hex = include_str!("../test_data/block_header_2_963_520.hex");
    let bytes = Vec::<u8>::from_hex(hex).unwrap();
    BlockHeader::consensus_decode(&bytes[..]).unwrap()
}

pub fn add_checksum(desc: &str) -> String {
    if desc.find('#').is_some() {
        desc.into()
    } else {
        format!("{}#{}", desc, desc_checksum(desc).unwrap())
    }
}

pub fn compute_fee_rate_without_discount_ct(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

pub fn compute_fee_rate(pset: &PartiallySignedTransaction) -> f32 {
    let vsize = pset.extract_tx().unwrap().discount_vsize();
    let fee_satoshi = pset.outputs().last().unwrap().amount.unwrap();
    1000.0 * (fee_satoshi as f32 / vsize as f32)
}

pub fn assert_fee_rate(fee_rate: f32, expected: Option<f32>) {
    let expected = expected.unwrap_or(DEFAULT_FEE_RATE);
    let toll = 0.45;
    assert!(fee_rate > expected * (1.0 - toll));
    assert!(fee_rate < expected * (1.0 + toll));
}

pub fn parse_code_from_markdown(markdown_input: &str, code_kind: &str) -> Vec<String> {
    let parser = pulldown_cmark::Parser::new(markdown_input);
    let mut result = vec![];
    let mut str = String::new();
    let mut active = false;

    for el in parser {
        match el {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(current)))
                if code_kind == current.as_ref() =>
            {
                active = true
            }
            Event::Text(t) => {
                if active {
                    str.push_str(t.as_ref())
                }
            }
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(current)))
                if code_kind == current.as_ref() =>
            {
                result.push(str.clone());
                str.clear();
                active = false;
            }
            _ => (),
        }
    }

    result
}

/// Serialize and deserialize a PSET
///
/// This allows us to catch early (de)serialization issues,
/// which can be hit in practice since PSETs are passed around as b64 strings.
pub fn pset_rt(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    PartiallySignedTransaction::from_str(&pset.to_string()).unwrap()
}

pub fn regtest_policy_asset() -> AssetId {
    AssetId::from_str("5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225").unwrap()
}

pub fn init_logging() {
    let _ = env_logger::try_init();
}

#[allow(dead_code)]
pub fn prune_proofs(pset: &PartiallySignedTransaction) -> PartiallySignedTransaction {
    let mut pset = pset.clone();
    for i in pset.inputs_mut() {
        if let Some(utxo) = &mut i.witness_utxo {
            utxo.witness = TxOutWitness::default();
        }
        if let Some(tx) = &mut i.non_witness_utxo {
            tx.output
                .iter_mut()
                .for_each(|o| o.witness = Default::default());
        }
    }
    for o in pset.outputs_mut() {
        o.value_rangeproof = None;
        o.asset_surjection_proof = None;
        o.blind_value_proof = None;
        o.blind_asset_proof = None;
    }
    pset
}

pub fn generate_mnemonic() -> String {
    let mut bytes = [0u8; 16];
    thread_rng().fill(&mut bytes);
    bip39::Mnemonic::from_entropy(&bytes).unwrap().to_string()
}

pub fn generate_slip77() -> String {
    let mut bytes = [0u8; 32];
    thread_rng().fill(&mut bytes);
    bytes.to_hex()
}

pub fn generate_view_key() -> String {
    let mut bytes = [0u8; 32];
    thread_rng().fill(&mut bytes);
    bytes.to_hex()
}

pub fn generate_xprv() -> Xpriv {
    let mut seed = [0u8; 16];
    thread_rng().fill(&mut seed);
    Xpriv::new_master(Network::Regtest, &seed).unwrap()
}

pub fn n_issuances(details: &lwk_common::PsetDetails) -> usize {
    details.issuances.iter().filter(|e| e.is_issuance()).count()
}

pub fn n_reissuances(details: &lwk_common::PsetDetails) -> usize {
    details
        .issuances
        .iter()
        .filter(|e| e.is_reissuance())
        .count()
}

pub fn asset_blinding_factor_test_vector() -> AssetBlindingFactor {
    AssetBlindingFactor::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000001",
    )
    .unwrap()
}

pub fn value_blinding_factor_test_vector() -> ValueBlindingFactor {
    ValueBlindingFactor::from_hex(
        "0000000000000000000000000000000000000000000000000000000000000002",
    )
    .unwrap()
}

pub fn txid_test_vector() -> Txid {
    Txid::from_str("0000000000000000000000000000000000000000000000000000000000000003").unwrap()
}

pub fn tx_out_secrets_test_vector() -> TxOutSecrets {
    elements::TxOutSecrets::new(
        regtest_policy_asset(),
        asset_blinding_factor_test_vector(),
        1000,
        value_blinding_factor_test_vector(),
    )
}

pub fn tx_out_secrets_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/tx_out_secrets_test_vector.hex")).unwrap()
}

pub fn update_test_vector_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector.hex")).unwrap()
}

pub fn update_test_vector_v1_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!("../test_data/update_test_vector_v1.hex")).unwrap()
}

pub fn update_test_vector_2_bytes() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector.bin").to_vec()
}

/// An update (serialized v1) with 63 transactions on liquid testnet wallet defined by [`wollet_descriptor_many_transactions`]
pub fn update_test_vector_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_many_txs.bin").to_vec()
}

/// An update (serialized v2) after [`update_test_vector_many_transactions`]
pub fn update_v2_test_vector_after_many_transactions() -> Vec<u8> {
    include_bytes!("../test_data/update_v2_after_many_txs.bin").to_vec()
}

pub fn update_test_vector_encrypted_bytes() -> Vec<u8> {
    Vec::<u8>::from_hex(include_str!(
        "../test_data/update_test_vector_encrypted.hex"
    ))
    .unwrap()
}

pub fn update_test_vector_encrypted_base64() -> String {
    include_str!("../test_data/update_test_vector/update.base64").to_string()
}

pub fn update_test_vector_encrypted_bytes2() -> Vec<u8> {
    include_bytes!("../test_data/update_test_vector/000000000000").to_vec()
}

pub fn wollet_descriptor_string2() -> String {
    include_str!("../test_data/update_test_vector/desc").to_string()
}

pub fn wollet_descriptor_string() -> String {
    include_str!("../test_data/update_test_vector/desc2").to_string()
}

pub fn wollet_descriptor_many_transactions() -> &'static str {
    "ct(slip77(ac53739ddde9fdf6bba3dbc51e989b09aa8c9cdce7b7d7eddd49cec86ddf71f7),elwpkh([93970d14/84'/1'/0']tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))#u0y4axgs"
}

/// A 3 of 5 descriptor and a vector of partially signed transactions to combine 1 sig each
pub fn psets_to_combine() -> (String, Vec<PartiallySignedTransaction>) {
    let c = |s: &str| PartiallySignedTransaction::from_str(s).unwrap();
    let ps = vec![
        c(include_str!("../test_data/pset_combine/s1_pset.base64")),
        c(include_str!("../test_data/pset_combine/s2_pset.base64")),
        c(include_str!("../test_data/pset_combine/s3_pset.base64")),
        c(include_str!("../test_data/pset_combine/s4_pset.base64")),
        c(include_str!("../test_data/pset_combine/s5_pset.base64")),
    ];
    let d = include_str!("../test_data/pset_combine/desc");
    (d.to_string(), ps)
}

pub fn descriptor_pset_usdt_no_contracts() -> &'static str {
    include_str!("../test_data/pset_usdt/desc")
}

/// Pset created with descriptor [`descriptor_pset_usdt_no_contracts`] containing mainnet USDt but without contract info
pub fn pset_usdt_no_contracts() -> &'static str {
    include_str!("../test_data/pset_usdt/pset_usdt_no_contracts.base64")
}

pub fn pset_usdt_with_contract() -> &'static str {
    include_str!("../test_data/pset_usdt/pset_usdt_with_contract.base64")
}

#[cfg(test)]
mod test {

    use crate::parse_code_from_markdown;

    #[test]
    fn test_parse_code_from_markdown() {
        let mkdown = r#"
```python
python
code
```
```rust
rust
code
```

```python
some more
python code
"#;
        let res = parse_code_from_markdown(mkdown, "python");
        assert_eq!(
            res,
            vec![
                "python\ncode\n".to_string(),
                "some more\npython code\n".to_string()
            ]
        );

        let res = parse_code_from_markdown(mkdown, "rust");
        assert_eq!(res, vec!["rust\ncode\n".to_string()])
    }
}
