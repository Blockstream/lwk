use crate::test_wollet::*;
use lwk_test_util::*;
use lwk_wollet::clients::blocking::BlockchainBackend;
use lwk_wollet::*;
use std::collections::HashMap;

#[test]
fn test_tx_inclusion_proofs_electrum() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let client = test_client_electrum(&env.electrum_url());

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc_str = format!("ct({view_key},elwpkh({}/<0;1>/*))", signer.xpub());

    let mut wallet = TestWollet::new(client, &desc_str);

    let txid1 = wallet.fund(&env, 10_000, None, None);
    let txid2 = wallet.fund(&env, 20_000, None, None);
    env.elementsd_generate(1);
    wait_for_tx_confirmation(&mut wallet.wollet, &mut wallet.client, &txid2);

    // Both txs are mined in the same block.
    let height = wallet.get_tx(&txid1).height.unwrap();
    assert_eq!(wallet.get_tx(&txid2).height, Some(height));
    let root = wallet
        .client
        .get_headers(&[height], &HashMap::new())
        .unwrap()[0]
        .merkle_root;

    let txs = [(txid1, height), (txid2, height)];
    let proofs = wallet.client.get_tx_inclusion_proofs(&txs).unwrap();
    assert_eq!(proofs.len(), txs.len());

    assert!(verify_tx_inclusion(txid1, &proofs[0], root));
    assert!(verify_tx_inclusion(txid2, &proofs[1], root));
    assert!(!verify_tx_inclusion(txid1, &proofs[1], root));
    assert!(!verify_tx_inclusion(txid2, &proofs[0], root));
}
