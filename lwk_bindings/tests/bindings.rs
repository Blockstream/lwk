#[cfg(feature = "foreign_bindings")]
uniffi::build_foreign_language_testcases!(
    "tests/bindings/custom_persister.py",
    "tests/bindings/external_unblind.py",
    "tests/bindings/list_transactions.py",
    "tests/bindings/issue_asset.py",
    "tests/bindings/send_asset.py",
    "tests/bindings/send_transaction.py",
    "tests/bindings/test_env.py",
    "tests/bindings/multisig.py",
    "tests/bindings/amp0-setup.py",
    "tests/bindings/amp0-daily-ops.py",
    "tests/bindings/amp2.py",
    "tests/bindings/lightning.py",
    "tests/bindings/list_transactions.kts",
    "tests/bindings/list_transactions.swift",
    "tests/bindings/pset_details.py",
    "tests/bindings/manual_coin_selection.py",
    "tests/bindings/liquidex.py",
    "tests/bindings/p2sh-multi.py",
    "tests/bindings/chained_reissuances.py",
    "tests/bindings/external_utxos.py",
    "tests/bindings/send_explicit.py",
    "tests/bindings/basics.py",
    "tests/bindings/bip85.py",
    "tests/bindings/dwid.py",
    "tests/bindings/payment_instructions.py"
);

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
uniffi::build_foreign_language_testcases!(
    "tests/bindings/simplicity_p2pk.py",
    "tests/bindings/simplicity_p2pk_regtest.py"
);
