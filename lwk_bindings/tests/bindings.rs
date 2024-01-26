#[cfg(feature = "foreign_bindings")]
uniffi::build_foreign_language_testcases!("tests/bindings/python.py", "tests/bindings/kotlin.kts");
