use lwk_common::Signer;
use lwk_simplicity::lending::OfferDetails;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::blocking::EsploraClient;
use lwk_wollet::elements::AssetId;
use std::str::FromStr;

use lwk_common::Network;
use lwk_signer::SwSigner;
use lwk_simplicity::lending::LendingSession;
use lwk_test_util::generate_mnemonic;
use lwk_test_util::generate_view_key;
use lwk_test_util::TestEnvBuilder;
use lwk_wollet::WolletBuilder;
use lwk_wollet::WolletDescriptor;

#[test]
#[should_panic]
fn test_borrow_flow() {
    let env = TestEnvBuilder::from_env().with_esplora().build();

    let client = EsploraClient::new(&env.esplora_url(), Network::default_regtest()).unwrap();
    let network = env.elementsd_network();

    // Create borrower
    let mnemonic = generate_mnemonic();
    let signer_borrower = SwSigner::new(&mnemonic, false).unwrap();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer_borrower.xpub());
    let wd = WolletDescriptor::from_str(&desc).unwrap();
    let wollet_borrower = WolletBuilder::new(network, wd.clone()).build().unwrap();

    // Create lender
    let mnemonic = generate_mnemonic();
    let signer_lender = SwSigner::new(&mnemonic, false).unwrap();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer_lender.xpub());
    let wd = WolletDescriptor::from_str(&desc).unwrap();
    let _wollet_lender = WolletBuilder::new(network, wd.clone()).build().unwrap();

    // Create lending session
    let session = LendingSession::builder(network, wd).build().unwrap();

    // Create, sign and broadcast transaction for borrow preparations for given wollet
    let mut pset = session
        .borrower_prepare(&wollet_borrower)
        .unwrap()
        .inner()
        .clone();
    let sig_added = signer_borrower.sign(&mut pset).unwrap();
    // only one sign -- on utxo which are used for fee and issuance.
    assert_eq!(sig_added, 1);
    let tx = wollet_borrower.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();

    // Create borrow details
    let borrow_details = OfferDetails {
        principal_asset_id: AssetId::from_byte_array([0; 32]),
        principal_amount: 10000,
        collateral_asset_id: *network.policy_asset(),
        collateral_amount: 200000,
        // 60 blocks after the current one
        loan_expiration_time: env.elementsd_height() as u32 + 60,
        // 20 % interest rate
        principal_interest_rate: 2_000,
    };

    // Create, sign and broadcast borrow offer
    let mut pset = session
        .borrower_create_offer(&wollet_borrower, borrow_details)
        .unwrap()
        .inner()
        .clone();
    let _sig_added = signer_borrower.sign(&mut pset).unwrap();
    let tx = wollet_borrower.finalize(&mut pset).unwrap();
    client.broadcast(&tx).unwrap();
}
