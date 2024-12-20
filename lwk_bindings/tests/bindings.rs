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
    "tests/bindings/amp2.py",
    "tests/bindings/list_transactions.kts",
    "tests/bindings/list_transactions.swift",
);
