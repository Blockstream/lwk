use elements_miniscript::elements::bitcoin::bip32::DerivationPath;
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use elements_miniscript::elements::AddressParams;
use lwk_containers::testcontainers::clients;
use lwk_containers::{LedgerEmulator, LEDGER_EMULATOR_PORT};
use lwk_ledger::*;

#[test]
fn test_ledger_commands() {
    let docker = clients::Cli::default();
    let ledger = LedgerEmulator::new().expect("test");
    let container = docker.run(ledger);
    let port = container.get_host_port_ipv4(LEDGER_EMULATOR_PORT);
    let client = Ledger::new(port).client;
    let (name, version, _flags) = client.get_version().unwrap();
    assert_eq!(version, "2.2.3");
    assert_eq!(name, "Liquid Regtest");
    let fingerprint = client.get_master_fingerprint().unwrap();
    assert_eq!(fingerprint.to_string(), "f5acc2fd");

    let path: DerivationPath = "m/44h/1h/0h".parse().unwrap();
    let xpub = client.get_extended_pubkey(&path, false).unwrap();
    assert_eq!(xpub.to_string(), "tpubDCwYjpDhUdPGP5rS3wgNg13mTrrjBuG8V9VpWbyptX6TRPbNoZVXsoVUSkCjmQ8jJycjuDKBb9eataSymXakTTaGifxR6kmVsfFehH1ZgJT");

    let message_path: DerivationPath = "m/44h/1h/0h/0/0".parse().unwrap();
    let message = [3u8; 32];
    let (prefix, sig) = client.sign_message(&message, &message_path).unwrap();
    assert_eq!(prefix, 27 + 4);
    assert_eq!(
        sig.to_string(),
        "3044022031e78eaebca6af2157bff0ddf9ed40498c8b9d4b184bfb0ff893959beb6d794c022033e0ecd424b5d4a31e946e29f06d2da185c2ae5c7d3d63a71dd6115ec5516464",
    );
    // TODO: verify

    let master_blinding_key = client.get_master_blinding_key().unwrap();
    assert_eq!(
        master_blinding_key.to_string(),
        "0c11648c2c6df4f9dacdb4c8d35d6166d94cea2b9ad37833a82210bb7c9f5fb4"
    );

    let version = Version::V2;
    let wpk0 = WalletPubKey::from(((fingerprint, path), xpub));
    use std::str::FromStr;
    let wpk1 = WalletPubKey::from_str("[76223a6e/48'/1'/0'/1']tpubDE7NQymr4AFtcJXi9TaWZtrhAdy8QyKmT4U6b9qYByAxCzoyMJ8zw5d8xVLVpbTRAEqP8pVUxjLE2vDt1rSFjaiS8DSz1QcNZ8D1qxUMx1g").unwrap();
    let keys = vec![wpk0, wpk1];

    let wallet_policy = WalletPolicy::new_multisig(
        "testliquid".to_string(),
        version,
        AddressType::NestedSegwit,
        2,
        keys,
        false,
        Some(format!("slip77({master_blinding_key})")),
    )
    .unwrap();
    let (id, hmac) = client.register_wallet(&wallet_policy).unwrap();

    assert_eq!(id, wallet_policy.id());

    let params = &AddressParams::ELEMENTS;
    let address = client
        .get_wallet_address(
            &wallet_policy,
            Some(&hmac),
            false, // change
            0,     // address index
            false, // display
            params,
        )
        .unwrap();
    assert_eq!(
        address.to_string(),
        "AzpwDWqFZA5sMX2TK33kiNrn115ChnERKd2G2J4rffpRPcAnhnZ4EpYyJdjJ234ErsWrEF5bwtoyjpXx"
    );

    // Single sig, no need to register the wallet
    let version = Version::V2;
    let path: DerivationPath = "m/84h/1h/0h".parse().unwrap();
    let xpub = client.get_extended_pubkey(&path, false).unwrap();
    let wpk0 = WalletPubKey::from(((fingerprint, path), xpub));
    let ss_keys = vec![wpk0];
    let desc = format!("ct(slip77({master_blinding_key}),wpkh(@0/**))");
    // For wallets that do not require registration, name must be empty
    let ss = WalletPolicy::new("".to_string(), version, desc, ss_keys.clone());
    let address = client
        .get_wallet_address(
            &ss, None,  // hmac
            false, // change
            0,     // address index
            false, // display
            params,
        )
        .unwrap();
    let expected = "el1qqvk6gl0lgs80w8rargdqyfsl7f0llsttzsx8gd4fz262cjnt0uxh6y68aq4qx76ahvuvlrz8t8ey9v04clsf58w045gzmxga3";
    assert_eq!(address.to_string(), expected);

    let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
    let desc = format!("ct({view_key},wpkh(@0/**))");
    let ss_view = WalletPolicy::new("".to_string(), version, desc, ss_keys);
    let address = client
        .get_wallet_address(
            &ss_view, None,  // hmac
            false, // change
            0,     // address index
            false, // display
            params,
        )
        .unwrap();
    let expected = "el1qq2fk6wmtxd49cymtpprte3ue5x4elp99s5zltzhy8pwjf0pqw7qeyy68aq4qx76ahvuvlrz8t8ey9v04clsf503tn8tvv859j";
    assert_eq!(address.to_string(), expected);

    let pset_b64 = include_str!("../tests/data/pset_ledger.base64");
    let pset: PartiallySignedTransaction = pset_b64.parse().unwrap();

    let sigs = client
        .sign_psbt(
            &pset, &ss_view, None, // hmac
        )
        .unwrap();
    assert_eq!(sigs.len(), 1);
    // Signed the first input
    assert_eq!(sigs[0].0, 0);
    // From the Liquid Ledger App test vectors
    let expected = elements_miniscript::bitcoin::ecdsa::Signature::from_str("3044022071965f8315a264773d8e635fb5bb8dfdb425b849b7aaafa8f1dcf1356e87947a02202eae7f9bdb1f00af3d1662a10b8efc82f9e7ecb1fc4f76a0b7905dab4fc6358801").unwrap();
    let sig = match sigs[0].1 {
        PartialSignature::Sig(_, sig) => sig,
        _ => panic!("unexpected sig"),
    };
    assert_eq!(sig, expected);
}

#[cfg(feature = "serial")]
#[ignore = "requires hardware ledger connected via usb"]
#[test]
fn test_physical_device() {
    let client = Ledger::new_hid().client;
    let (name, version, _flags) = client.get_version().unwrap();
    assert_eq!(name, "BOLOS");
    assert_eq!(version, "1.5.5");
}

#[cfg(feature = "asyncr")]
#[tokio::test]
async fn test_asyncr_ledger() {
    let docker = clients::Cli::default();
    let ledger = LedgerEmulator::new().expect("test");
    let container = docker.run(ledger);
    let port = container.get_host_port_ipv4(LEDGER_EMULATOR_PORT);
    let ledger = asyncr::Ledger::new(port);
    let client = &ledger.client;
    let (name, version, _flags) = client.get_version().await.unwrap();
    assert_eq!(version, "2.2.3");
    assert_eq!(name, "Liquid Regtest");
    let fingerprint = client.get_master_fingerprint().await.unwrap();
    assert_eq!(fingerprint.to_string(), "f5acc2fd");

    let path: DerivationPath = "m/44h/1h/0h".parse().unwrap();
    let xpub = client.get_extended_pubkey(&path, false).await.unwrap();
    assert_eq!(xpub.to_string(), "tpubDCwYjpDhUdPGP5rS3wgNg13mTrrjBuG8V9VpWbyptX6TRPbNoZVXsoVUSkCjmQ8jJycjuDKBb9eataSymXakTTaGifxR6kmVsfFehH1ZgJT");

    let message_path: DerivationPath = "m/44h/1h/0h/0/0".parse().unwrap();
    let message = [3u8; 32];
    let (prefix, sig) = client.sign_message(&message, &message_path).await.unwrap();
    assert_eq!(prefix, 27 + 4);
    assert_eq!(
        sig.to_string(),
        "3044022031e78eaebca6af2157bff0ddf9ed40498c8b9d4b184bfb0ff893959beb6d794c022033e0ecd424b5d4a31e946e29f06d2da185c2ae5c7d3d63a71dd6115ec5516464",
    );
    // TODO: verify

    let master_blinding_key = client.get_master_blinding_key().await.unwrap();
    assert_eq!(
        master_blinding_key.to_string(),
        "0c11648c2c6df4f9dacdb4c8d35d6166d94cea2b9ad37833a82210bb7c9f5fb4"
    );

    let version = Version::V2;
    let wpk0 = WalletPubKey::from(((fingerprint, path), xpub));
    use std::str::FromStr;
    let wpk1 = WalletPubKey::from_str("[76223a6e/48'/1'/0'/1']tpubDE7NQymr4AFtcJXi9TaWZtrhAdy8QyKmT4U6b9qYByAxCzoyMJ8zw5d8xVLVpbTRAEqP8pVUxjLE2vDt1rSFjaiS8DSz1QcNZ8D1qxUMx1g").unwrap();
    let keys = vec![wpk0, wpk1];

    let wallet_policy = WalletPolicy::new_multisig(
        "testliquid".to_string(),
        version,
        AddressType::NestedSegwit,
        2,
        keys,
        false,
        Some(format!("slip77({master_blinding_key})")),
    )
    .unwrap();
    let (id, hmac) = client.register_wallet(&wallet_policy).await.unwrap();

    assert_eq!(id, wallet_policy.id());

    let params = &AddressParams::ELEMENTS;
    let address = client
        .get_wallet_address(
            &wallet_policy,
            Some(&hmac),
            false, // change
            0,     // address index
            false, // display
            params,
        )
        .await
        .unwrap();
    assert_eq!(
        address.to_string(),
        "AzpwDWqFZA5sMX2TK33kiNrn115ChnERKd2G2J4rffpRPcAnhnZ4EpYyJdjJ234ErsWrEF5bwtoyjpXx"
    );

    // Single sig, no need to register the wallet
    let version = Version::V2;
    let path: DerivationPath = "m/84h/1h/0h".parse().unwrap();
    let xpub = client.get_extended_pubkey(&path, false).await.unwrap();
    let wpk0 = WalletPubKey::from(((fingerprint, path), xpub));
    let ss_keys = vec![wpk0];
    let desc = format!("ct(slip77({master_blinding_key}),wpkh(@0/**))");
    let ss = WalletPolicy::new("".to_string(), version, desc, ss_keys.clone());
    let address = client
        .get_wallet_address(
            &ss, None,  // hmac
            false, // change
            0,     // address index
            false, // display
            params,
        )
        .await
        .unwrap();
    let expected = "el1qqvk6gl0lgs80w8rargdqyfsl7f0llsttzsx8gd4fz262cjnt0uxh6y68aq4qx76ahvuvlrz8t8ey9v04clsf58w045gzmxga3";
    assert_eq!(address.to_string(), expected);

    let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
    let desc = format!("ct({view_key},wpkh(@0/**))");
    let ss_view = WalletPolicy::new("".to_string(), version, desc, ss_keys);
    let address = client
        .get_wallet_address(
            &ss_view, None,  // hmac
            false, // change
            0,     // address index
            false, // display
            params,
        )
        .await
        .unwrap();
    let expected = "el1qq2fk6wmtxd49cymtpprte3ue5x4elp99s5zltzhy8pwjf0pqw7qeyy68aq4qx76ahvuvlrz8t8ey9v04clsf503tn8tvv859j";
    assert_eq!(address.to_string(), expected);

    let pset_b64 = include_str!("../tests/data/pset_ledger.base64");
    let pset: PartiallySignedTransaction = pset_b64.parse().unwrap();

    let sigs = client
        .sign_psbt(
            &pset, &ss_view, None, // hmac
        )
        .await
        .unwrap();
    assert_eq!(sigs.len(), 1);
    // Signed the first input
    assert_eq!(sigs[0].0, 0);
    // From the Liquid Ledger App test vectors
    let expected = elements_miniscript::bitcoin::ecdsa::Signature::from_str("3044022071965f8315a264773d8e635fb5bb8dfdb425b849b7aaafa8f1dcf1356e87947a02202eae7f9bdb1f00af3d1662a10b8efc82f9e7ecb1fc4f76a0b7905dab4fc6358801").unwrap();
    let sig = match sigs[0].1 {
        PartialSignature::Sig(_, sig) => sig,
        _ => panic!("unexpected sig"),
    };
    assert_eq!(sig, expected);
}
