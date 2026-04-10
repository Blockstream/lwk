#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
use std::env;

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
use std::fs;

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
use std::path::PathBuf;

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
use std::process::Command;

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
use camino::Utf8PathBuf;

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
use uniffi_bindgen::{
    bindings::{KotlinBindingGenerator, PythonBindingGenerator},
    library_mode::generate_bindings,
    BindgenCrateConfigSupplier, EmptyCrateConfigSupplier,
};

#[cfg(feature = "foreign_bindings")]
uniffi::build_foreign_language_testcases!(
    "tests/bindings/custom_store.py",
    "tests/bindings/wollet_builder.py",
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
    "tests/bindings/drain_lbtc.py",
    "tests/bindings/payment_instructions.py",
    "tests/bindings/fallback_client.py",
    "tests/bindings/serde_roundtrip.py",
);

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
uniffi::build_foreign_language_testcases!(
    "tests/bindings/simplicity_p2pk.py",
    "tests/bindings/simplicity_taproot_builder.py",
    "tests/bindings/simplicity_p2pk_regtest.py",
    "tests/bindings/simplicity_options_regtest.py"
);

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
fn to_utf8_path(path: PathBuf) -> Result<Utf8PathBuf, Box<dyn std::error::Error>> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| format!("path is not valid UTF-8: {}", path.display()).into())
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
struct LwkBindingsConfigSupplier {
    manifest_dir: Utf8PathBuf,
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
impl BindgenCrateConfigSupplier for LwkBindingsConfigSupplier {
    fn get_toml_path(&self, _crate_name: &str) -> Option<Utf8PathBuf> {
        Some(self.manifest_dir.join("uniffi.toml"))
    }
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
fn run_simplicity_python_test(script_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir
        .parent()
        .expect("workspace root")
        .join("target");
    let profile = env::var("PROFILE").unwrap_or_else(|_| String::from("debug"));
    let lib_name = if cfg!(target_os = "windows") {
        "lwk.dll"
    } else if cfg!(target_os = "macos") {
        "liblwk.dylib"
    } else {
        "liblwk.so"
    };
    let cdylib_path = target_dir.join(&profile).join("deps").join(lib_name);
    if !cdylib_path.exists() {
        return Err(format!("missing cdylib: {}", cdylib_path.display()).into());
    }

    let temp_root = env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| target_dir.join("tmp"));
    fs::create_dir_all(&temp_root)?;
    let temp_dir = tempfile::Builder::new()
        .prefix("lwk-bindings-python-")
        .tempdir_in(&temp_root)?;
    let out_dir = temp_dir.path().to_path_buf();
    let copied_cdylib = out_dir.join(lib_name);
    fs::copy(&cdylib_path, &copied_cdylib)?;

    let out_dir_utf8 = to_utf8_path(out_dir.clone())?;
    let copied_cdylib_utf8 = to_utf8_path(copied_cdylib)?;
    generate_bindings(
        &copied_cdylib_utf8,
        None,
        &PythonBindingGenerator,
        &EmptyCrateConfigSupplier,
        None,
        &out_dir_utf8,
        false,
    )?;

    let script_path = manifest_dir.join(script_file).canonicalize()?;
    let pythonpath = env::var_os("PYTHONPATH").unwrap_or_default();
    let pythonpath =
        env::join_paths(env::split_paths(&pythonpath).chain(std::iter::once(out_dir.clone())))?;

    let status = Command::new("python3")
        .current_dir(&out_dir)
        .env("PYTHONPATH", pythonpath)
        .arg(script_path)
        .status()?;
    if !status.success() {
        return Err("running `python3` failed".into());
    }

    Ok(())
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
fn run_simplicity_kotlin_test(script_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir
        .parent()
        .expect("workspace root")
        .join("target");
    let profile = env::var("PROFILE").unwrap_or_else(|_| String::from("debug"));
    let lib_name = if cfg!(target_os = "windows") {
        "lwk.dll"
    } else if cfg!(target_os = "macos") {
        "liblwk.dylib"
    } else {
        "liblwk.so"
    };
    let cdylib_path = target_dir.join(&profile).join("deps").join(lib_name);
    if !cdylib_path.exists() {
        return Err(format!("missing cdylib: {}", cdylib_path.display()).into());
    }

    let temp_root = env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| target_dir.join("tmp"));
    fs::create_dir_all(&temp_root)?;
    let temp_dir = tempfile::Builder::new()
        .prefix("lwk-bindings-kotlin-")
        .tempdir_in(&temp_root)?;
    let out_dir = temp_dir.path().to_path_buf();
    let copied_cdylib = out_dir.join(lib_name);
    fs::copy(&cdylib_path, &copied_cdylib)?;

    let out_dir_utf8 = to_utf8_path(out_dir.clone())?;
    let copied_cdylib_utf8 = to_utf8_path(copied_cdylib)?;
    generate_bindings(
        &copied_cdylib_utf8,
        None,
        &KotlinBindingGenerator,
        &LwkBindingsConfigSupplier {
            manifest_dir: to_utf8_path(manifest_dir.clone())?,
        },
        None,
        &out_dir_utf8,
        false,
    )?;

    let generated_root = out_dir.join("generated");
    fs::create_dir_all(&generated_root)?;
    if out_dir.join("lwk").exists() {
        fs::rename(out_dir.join("lwk"), generated_root.join("lwk"))?;
    } else if out_dir.join("uniffi").exists() {
        fs::rename(out_dir.join("uniffi"), generated_root.join("uniffi"))?;
    } else {
        return Err(format!("missing generated Kotlin bindings in {}", out_dir.display()).into());
    }

    let script_path = manifest_dir.join(script_file).canonicalize()?;
    let program_dir = out_dir.join("program");
    fs::create_dir_all(&program_dir)?;
    fs::write(
        program_dir.join("WalletAbiProviderSmoke.kt"),
        wrap_kotlin_script_as_main(&fs::read_to_string(&script_path)?)?,
    )?;

    fs::write(
        out_dir.join("settings.gradle.kts"),
        r#"pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        mavenCentral()
    }
}

rootProject.name = "lwk_bindings_foreign_test"
"#,
    )?;
    fs::write(
        out_dir.join("build.gradle.kts"),
        format!(
            r#"import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {{
    kotlin("jvm") version "2.3.20"
    kotlin("plugin.serialization") version "2.3.20"
}}

dependencies {{
    implementation(kotlin("stdlib"))
    implementation("net.java.dev.jna:jna:5.18.1")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-core:1.10.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.10.0")
}}

kotlin {{
    compilerOptions {{
        jvmTarget.set(JvmTarget.JVM_17)
    }}
}}

sourceSets {{
    main {{
        kotlin.srcDir("generated")
        kotlin.srcDir("program")
    }}
}}

tasks.register<JavaExec>("runSmoke") {{
    dependsOn(tasks.named("classes"))
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass.set("WalletAbiProviderSmokeKt")
    jvmArgs(
        "-ea",
        "-Djna.library.path={}",
        "-Djava.library.path={}",
    )
}}
"#,
            out_dir.display(),
            out_dir.display(),
        ),
    )?;

    let gradlew = manifest_dir.join("android_bindings").join("gradlew");
    if !gradlew.exists() {
        return Err(format!("missing Gradle wrapper: {}", gradlew.display()).into());
    }
    let status = Command::new(gradlew)
        .current_dir(&out_dir)
        .arg("-p")
        .arg(&out_dir)
        .arg("runSmoke")
        .arg("--no-daemon")
        .status()?;
    if !status.success() {
        return Err("running Gradle Kotlin provider smoke failed".into());
    }

    Ok(())
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
fn wrap_kotlin_script_as_main(script: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut imports = Vec::new();
    let mut body = Vec::new();
    let mut in_preamble = true;

    for line in script.lines() {
        if in_preamble && line.starts_with("import ") {
            imports.push(line.to_string());
            continue;
        }
        if in_preamble && line.trim().is_empty() {
            continue;
        }
        in_preamble = false;
        body.push(line);
    }

    if body.is_empty() {
        return Err("Kotlin smoke script body is empty".into());
    }

    let mut wrapped = String::new();
    if !imports.is_empty() {
        wrapped.push_str(&imports.join("\n"));
        wrapped.push_str("\n\n");
    }
    wrapped.push_str("fun main() {\n");
    for line in body {
        if line.is_empty() {
            wrapped.push('\n');
        } else {
            wrapped.push_str("    ");
            wrapped.push_str(line);
            wrapped.push('\n');
        }
    }
    wrapped.push_str("}\n");

    Ok(wrapped)
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
#[test]
fn uniffi_foreign_language_testcase_wallet_abi_schema_py() -> Result<(), Box<dyn std::error::Error>>
{
    run_simplicity_python_test("tests/bindings/wallet_abi_schema.py")
}

#[cfg(all(feature = "foreign_bindings", feature = "simplicity"))]
#[test]
fn uniffi_foreign_language_testcase_wallet_abi_provider_kts(
) -> Result<(), Box<dyn std::error::Error>> {
    run_simplicity_kotlin_test("tests/bindings/wallet_abi_provider.kts")
}
