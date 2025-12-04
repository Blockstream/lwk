use crate::test_wollet::*;
use lwk_common::Signer;
use lwk_test_util::*;
use lwk_wollet::clients::blocking::EsploraClient;
use lwk_wollet::registry::{blocking::Registry, Contract, Entity, RegistryPost};
use lwk_wollet::ElementsNetwork;

#[test]
fn test_registry() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .with_registry()
        .build();

    // Create wallet
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());
    let url = env.esplora_url();
    let client = EsploraClient::new(&url, ElementsNetwork::default_regtest()).unwrap();
    let mut w = TestWollet::new(client, &desc);
    w.fund_btc(&env);

    // Issue an asset
    let contract = Contract {
        entity: Entity::Domain("liquidtestnet.com".into()),
        issuer_pubkey: [2; 33].into(),
        name: "Test Asset".into(),
        precision: 0,
        ticker: "TEST".into(),
        version: 0,
    };

    let mut pset = w
        .tx_builder()
        .issue_asset(10, None, 0, None, Some(contract.clone()))
        .unwrap()
        .finish()
        .unwrap();
    let asset_id = pset.inputs()[0].issuance_ids().0;

    let sigs_added = signer.sign(&mut pset).unwrap();
    assert_eq!(sigs_added, 1);

    w.send(&mut pset);
    env.elementsd_generate(2);

    // Publish on asset registry
    let registry = Registry::new(&env.registry_url()).unwrap();
    let payload = RegistryPost::new(contract.clone(), asset_id);
    registry.post(&payload).unwrap();

    // Fetch from asset registry
    let data = registry.fetch(asset_id).unwrap();
    assert_eq!(data.contract, contract);
}
