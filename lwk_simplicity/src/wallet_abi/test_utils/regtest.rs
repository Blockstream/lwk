use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, RwLock};

use lwk_common::Network;
use lwk_signer::SwSigner;
use lwk_test_util::{regtest_policy_asset, TestEnv, TestEnvBuilder};
use lwk_wollet::asyncr::EsploraClient;
use lwk_wollet::elements::{Address, AssetId};
use lwk_wollet::{ElementsNetwork, Wollet};

use crate::error::WalletAbiError;

use super::{TestSignerMeta, TestWalletMeta};

static TEST_ENV: LazyLock<Mutex<Option<Arc<RwLock<TestEnv>>>>> = LazyLock::new(|| Mutex::new(None));
static TEST_ENV_INIT_LOCK: Mutex<()> = Mutex::new(());
static SHUTDOWN_HOOK_REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();

#[cfg(unix)]
unsafe extern "C" {
    fn atexit(callback: extern "C" fn()) -> i32;
}

#[cfg(unix)]
extern "C" fn shutdown_node_running_on_exit() {
    let _ = shutdown_node_running();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFundingAsset {
    Lbtc,
    IssuedAsset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFundingResult {
    pub funded_asset_id: AssetId,
    pub funded_amount_sat: u64,
}

pub async fn build_runtime_parts_from_mnemonic(
    mnemonic: &str,
    network: Network,
    esplora_url: &str,
    wallet_data_dir: impl AsRef<Path>,
) -> Result<(TestSignerMeta, TestWalletMeta), WalletAbiError> {
    let signer_meta = TestSignerMeta::from_mnemonic(mnemonic, network)?;
    let descriptor = signer_meta.descriptor().clone();
    let elements_network = to_elements_network(network);

    let wallet_meta = TestWalletMeta::new(
        EsploraClient::new(elements_network, esplora_url),
        Wollet::with_fs_persist(elements_network, descriptor, wallet_data_dir).map_err(
            |error| WalletAbiError::InvalidResponse(format!("failed to create wallet: {error}")),
        )?,
    );
    wallet_meta.sync_wallet().await?;

    Ok((signer_meta, wallet_meta))
}

pub async fn build_runtime_parts_random(
    network: Network,
    esplora_url: &str,
    wallet_data_dir: impl AsRef<Path>,
) -> Result<(TestSignerMeta, TestWalletMeta), WalletAbiError> {
    let (signer, _mnemonic) = SwSigner::random(network.is_mainnet()).map_err(|error| {
        WalletAbiError::InvalidSignerConfig(format!("failed to create random signer: {error}"))
    })?;
    let signer_meta = TestSignerMeta::from_signer(signer, network)?;
    let descriptor = signer_meta.descriptor().clone();
    let elements_network = to_elements_network(network);

    let wallet_meta = TestWalletMeta::new(
        EsploraClient::new(elements_network, esplora_url),
        Wollet::with_fs_persist(elements_network, descriptor, wallet_data_dir).map_err(
            |error| WalletAbiError::InvalidResponse(format!("failed to create wallet: {error}")),
        )?,
    );
    wallet_meta.sync_wallet().await?;

    Ok((signer_meta, wallet_meta))
}

fn ensure_shutdown_hook_registered() -> Result<(), WalletAbiError> {
    let registration = SHUTDOWN_HOOK_REGISTRATION.get_or_init(|| {
        #[cfg(unix)]
        {
            // SAFETY: Callback uses C ABI and takes no arguments, matching `atexit`.
            let status = unsafe { atexit(shutdown_node_running_on_exit) };
            if status != 0 {
                return Err("failed to register regtest shutdown hook".to_string());
            }
        }

        Ok(())
    });

    registration
        .as_ref()
        .map(|_| ())
        .map_err(|message| WalletAbiError::InvalidResponse(message.clone()))
}

pub fn ensure_node_running() -> Result<(), WalletAbiError> {
    ensure_shutdown_hook_registered()?;

    if test_env_slot()?.is_some() {
        return Ok(());
    }

    let _guard = TEST_ENV_INIT_LOCK
        .lock()
        .map_err(|_| WalletAbiError::InvalidResponse("test env init lock poisoned".to_string()))?;

    if test_env_slot()?.is_some() {
        return Ok(());
    }

    let env = std::panic::catch_unwind(|| TestEnvBuilder::from_env().with_esplora().build())
        .map_err(|panic| {
            panic.downcast_ref::<String>().map_or_else(
                || {
                    WalletAbiError::InvalidResponse(
                        "failed to start regtest test environment".to_string(),
                    )
                },
                |message| {
                    WalletAbiError::InvalidResponse(format!(
                        "failed to start regtest test environment: {message}"
                    ))
                },
            )
        })?;

    let mut slot = TEST_ENV
        .lock()
        .map_err(|_| WalletAbiError::InvalidResponse("test env slot lock poisoned".to_string()))?;
    *slot = Some(Arc::new(RwLock::new(env)));

    Ok(())
}

pub fn shutdown_node_running() -> Result<(), WalletAbiError> {
    let env = {
        let mut slot = TEST_ENV.lock().map_err(|_| {
            WalletAbiError::InvalidResponse("test env slot lock poisoned".to_string())
        })?;
        slot.take()
    };
    drop(env);
    Ok(())
}

pub fn get_esplora_url() -> Result<String, WalletAbiError> {
    with_test_env_read(|env| Ok(env.esplora_url()))
}

pub fn mine_blocks(blocks: usize) -> Result<(), WalletAbiError> {
    let blocks_u32 = u32::try_from(blocks)
        .map_err(|_| WalletAbiError::InvalidRequest(format!("blocks must fit in u32: {blocks}")))?;
    with_test_env_read(|env| {
        env.elementsd_generate(blocks_u32);
        Ok(())
    })
}

pub fn fund_address(
    signer_address: &Address,
    asset: RuntimeFundingAsset,
    amount_sat: u64,
) -> Result<RuntimeFundingResult, WalletAbiError> {
    if amount_sat == 0 {
        return Err(WalletAbiError::InvalidRequest(
            "amount_sat must be > 0".to_string(),
        ));
    }

    with_test_env_read(|env| {
        let funded_asset_id = match asset {
            RuntimeFundingAsset::Lbtc => {
                let policy_asset = regtest_policy_asset();
                env.elementsd_sendtoaddress(signer_address, amount_sat, Some(policy_asset));
                env.elementsd_generate(1);
                policy_asset
            }
            RuntimeFundingAsset::IssuedAsset => {
                let issued_asset = env.elementsd_issueasset(amount_sat);
                env.elementsd_generate(1);
                env.elementsd_sendtoaddress(signer_address, amount_sat, Some(issued_asset));
                env.elementsd_generate(1);
                issued_asset
            }
        };

        Ok(RuntimeFundingResult {
            funded_asset_id,
            funded_amount_sat: amount_sat,
        })
    })
}

fn test_env() -> Result<Arc<RwLock<TestEnv>>, WalletAbiError> {
    ensure_node_running()?;
    test_env_slot()?
        .ok_or_else(|| WalletAbiError::InvalidResponse("test env failed to initialize".to_string()))
}

fn with_test_env_read<T>(
    f: impl FnOnce(&TestEnv) -> Result<T, WalletAbiError>,
) -> Result<T, WalletAbiError> {
    let env = test_env()?;
    let env = env
        .read()
        .map_err(|_| WalletAbiError::InvalidResponse("test env lock poisoned".to_string()))?;
    f(&env)
}

fn test_env_slot() -> Result<Option<Arc<RwLock<TestEnv>>>, WalletAbiError> {
    let slot = TEST_ENV
        .lock()
        .map_err(|_| WalletAbiError::InvalidResponse("test env slot lock poisoned".to_string()))?;
    Ok(slot.clone())
}

fn to_elements_network(network: Network) -> ElementsNetwork {
    match network {
        Network::Liquid => ElementsNetwork::Liquid,
        Network::TestnetLiquid => ElementsNetwork::LiquidTestnet,
        Network::LocaltestLiquid => ElementsNetwork::default_regtest(),
    }
}
