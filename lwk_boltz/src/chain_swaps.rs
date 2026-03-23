use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use aes_gcm_siv::Aes256GcmSiv;
use bip39::Mnemonic;
use boltz_client::boltz::{
    BoltzApiClientV2, ChainSwapDetails, ChainSwapStates, CreateChainRequest, CreateChainResponse,
    Side, SwapRestoreResponse, SwapRestoreType, SwapStatus, Webhook,
};
use boltz_client::fees::Fee;
use boltz_client::network::Chain;
use boltz_client::swaps::{
    BtcLikeTransaction, ChainClient, SwapScript, SwapTransactionParams, TransactionOptions,
};
use boltz_client::util::sleep;
use boltz_client::PublicKey;
use lwk_wollet::bitcoin::PublicKey as BitcoinPublicKey;
use lwk_wollet::elements;
use lwk_wollet::elements::bitcoin;

use crate::chain_data::{chain_from_str, to_chain_data, ChainSwapData, ChainSwapDataSerializable};
use crate::error::Error;
use crate::swap_state::SwapStateTrait;
use crate::DynStore;
use crate::SwapPersistence;
use crate::LIQUID_UNCOOPERATIVE_EXTRA;
use crate::{
    broadcast_tx_with_retry, derive_keypair, mnemonic_identifier, next_status,
    preimage_from_keypair, WAIT_TIME,
};
use crate::{BoltzSession, SwapState, SwapType};

pub struct LockupResponse {
    pub data: ChainSwapData,

    // unserializable fields
    lockup_script: SwapScript,
    claim_script: SwapScript,
    rx: tokio::sync::broadcast::Receiver<SwapStatus>,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    polling: bool,
    timeout_advance: Duration,
    store: Option<Arc<dyn DynStore>>,
    cipher: Option<Aes256GcmSiv>,
}

impl SwapPersistence for LockupResponse {
    fn serialize(&self) -> Result<String, Error> {
        let s: ChainSwapDataSerializable = self.data.clone().into();
        Ok(serde_json::to_string(&s)?)
    }

    fn swap_id(&self) -> &str {
        &self.data.create_chain_response.id
    }

    fn store_and_cipher(&self) -> Option<(Arc<dyn DynStore>, Aes256GcmSiv)> {
        match (self.store.as_ref(), self.cipher.as_ref()) {
            (Some(store), Some(cipher)) => Some((Arc::clone(store), cipher.clone())),
            _ => None,
        }
    }
}

impl BoltzSession {
    pub async fn btc_to_lbtc(
        &self,
        amount: u64,
        refund_address: &bitcoin::Address,
        claim_address: &elements::Address,
        webhook: Option<Webhook<ChainSwapStates>>,
    ) -> Result<LockupResponse, Error> {
        let from = self.btc_chain();
        let to = self.chain();
        self.onchain_swap(
            from,
            to,
            amount,
            &refund_address.to_string(),
            &claim_address.to_string(),
            webhook,
        )
        .await
    }

    pub async fn lbtc_to_btc(
        &self,
        amount: u64,
        refund_address: &elements::Address,
        claim_address: &bitcoin::Address,
        webhook: Option<Webhook<ChainSwapStates>>,
    ) -> Result<LockupResponse, Error> {
        let from = self.chain();
        let to = self.btc_chain();
        self.onchain_swap(
            from,
            to,
            amount,
            &refund_address.to_string(),
            &claim_address.to_string(),
            webhook,
        )
        .await
    }

    async fn onchain_swap(
        &self,
        from: Chain,
        to: Chain,
        amount: u64,
        refund_address: &str,
        claim_address: &str,
        webhook: Option<Webhook<ChainSwapStates>>,
    ) -> Result<LockupResponse, Error> {
        let (claim_key_index, claim_keys) = self.derive_next_keypair()?;
        let preimage = self.preimage(&claim_keys);
        let (refund_key_index, refund_keys) = self.derive_next_keypair()?;

        let claim_public_key = PublicKey {
            inner: claim_keys.public_key(),
            compressed: true,
        };
        let refund_public_key = PublicKey {
            inner: refund_keys.public_key(),
            compressed: true,
        };

        let create_chain_req = CreateChainRequest {
            from: from.to_string(),
            to: to.to_string(),
            preimage_hash: preimage.sha256,
            claim_public_key: Some(claim_public_key),
            refund_public_key: Some(refund_public_key),
            user_lock_amount: Some(amount),
            server_lock_amount: None,
            pair_hash: None,
            referral_id: self.referral_id.clone(),
            webhook,
        };

        let create_chain_response = self.api.post_chain_req(create_chain_req).await?;
        create_chain_response.validate(&claim_public_key, &refund_public_key, from, to)?;

        let swap_id = create_chain_response.id.clone();
        let lockup_script = SwapScript::chain_from_swap_resp(
            from,
            Side::Lockup,
            create_chain_response.lockup_details.clone(),
            refund_public_key,
        )?;
        let claim_script = SwapScript::chain_from_swap_resp(
            to,
            Side::Claim,
            create_chain_response.claim_details.clone(),
            claim_public_key,
        )?;

        self.ws.subscribe_swap(&swap_id).await?;
        let mut rx = self.ws.updates();
        let update = next_status(&mut rx, self.timeout, &swap_id, false).await?;
        let last_state = update.swap_state()?;

        let lockup_address = create_chain_response.lockup_details.lockup_address.clone();
        let expected_lockup_amount = create_chain_response.lockup_details.amount;
        // Fee is what you lock up minus what you receive on the claim side
        let fee = expected_lockup_amount.saturating_sub(create_chain_response.claim_details.amount);

        let (boltz_fee, claim_fee) = {
            let swap_info = self.swap_info.lock().await;
            match (from, to) {
                (Chain::Bitcoin(_), Chain::Liquid(_)) => {
                    match swap_info.chain_pairs.get_btc_to_lbtc_pair() {
                        Some(pair) => (
                            Some(pair.fees.boltz(amount)),
                            Some(pair.fees.claim_estimate()),
                        ),
                        None => (None, None),
                    }
                }
                (Chain::Liquid(_), Chain::Bitcoin(_)) => {
                    match swap_info.chain_pairs.get_lbtc_to_btc_pair() {
                        Some(pair) => (
                            Some(pair.fees.boltz(amount)),
                            Some(pair.fees.claim_estimate()),
                        ),
                        None => (None, None),
                    }
                }
                _ => (None, None),
            }
        };

        let store = self.clone_store();
        let cipher = if store.is_some() {
            Some(self.clone_cipher())
        } else {
            None
        };
        let response = LockupResponse {
            data: ChainSwapData {
                last_state,
                swap_type: SwapType::Chain,
                fee: Some(fee),
                boltz_fee,
                claim_fee,
                create_chain_response,
                claim_keys,
                refund_keys,
                preimage,
                lockup_address,
                expected_lockup_amount,
                claim_address: claim_address.to_string(),
                refund_address: refund_address.to_string(),
                claim_key_index,
                refund_key_index,
                mnemonic_identifier: mnemonic_identifier(&self.mnemonic)?,
                from_chain: from,
                to_chain: to,
                random_preimage: self.random_preimages,
                claim_txid: None,
                lockup_txid: None,
                refund_txid: None,
            },
            lockup_script,
            claim_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
            polling: self.polling,
            timeout_advance: self.timeout_advance,
            store,
            cipher,
        };

        // Persist swap data and add to pending list
        response.persist_and_add_to_pending()?;

        Ok(response)
    }

    pub async fn restore_lockup(
        &self,
        data: ChainSwapDataSerializable,
    ) -> Result<LockupResponse, Error> {
        let mut data = to_chain_data(data, &self.mnemonic)?;
        if data.lockup_txid.is_none() {
            data.lockup_txid =
                fetch_lockup_txid(self.api.as_ref(), &data.create_chain_response.id).await;
        }
        let from = data.from_chain;
        let to = data.to_chain;
        let claim_p = PublicKey {
            inner: data.claim_keys.public_key(),
            compressed: true,
        };
        let refund_p = PublicKey {
            inner: data.refund_keys.public_key(),
            compressed: true,
        };
        let lockup_script = SwapScript::chain_from_swap_resp(
            from,
            Side::Lockup,
            data.create_chain_response.lockup_details.clone(),
            refund_p,
        )?;
        let claim_script = SwapScript::chain_from_swap_resp(
            to,
            Side::Claim,
            data.create_chain_response.claim_details.clone(),
            claim_p,
        )?;
        let swap_id = data.create_chain_response.id.clone();
        let rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;

        let store = self.clone_store();
        let cipher = if store.is_some() {
            Some(self.clone_cipher())
        } else {
            None
        };
        let response = LockupResponse {
            data,
            lockup_script,
            claim_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
            polling: self.polling,
            timeout_advance: self.timeout_advance,
            store,
            cipher,
        };

        // If the swap was already in a terminal state, move it to completed
        if response.data.last_state.is_terminal() {
            response.move_to_completed()?;
        }

        Ok(response)
    }

    /// From the swaps returned by the boltz api via [`BoltzSession::swap_restore`]:
    ///
    /// - filter the BTC to LBTC swaps
    /// - add information from the session
    /// - return typed data
    ///
    /// The claim and refund addresses don't need to be the same used when creating the swap.
    pub async fn restorable_btc_to_lbtc_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &elements::Address,
        refund_address: &bitcoin::Address,
    ) -> Result<Vec<ChainSwapData>, Error> {
        let claim_address = claim_address.to_string();
        let refund_address = refund_address.to_string();
        swaps
            .iter()
            .filter(|e| matches!(e.swap_type, SwapRestoreType::Chain))
            .filter(|e| e.to == "L-BTC" && e.from == "BTC")
            .map(|e| {
                convert_swap_restore_response_to_chain_swap_data(
                    e,
                    &self.mnemonic,
                    &claim_address,
                    &refund_address,
                )
            })
            .collect()
    }

    /// From the swaps returned by the boltz api via [`BoltzSession::swap_restore`]:
    ///
    /// - filter the LBTC to BTC swaps
    /// - add information from the session
    /// - return typed data
    ///
    /// The claim and refund addresses don't need to be the same used when creating the swap.
    pub async fn restorable_lbtc_to_btc_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &bitcoin::Address,
        refund_address: &elements::Address,
    ) -> Result<Vec<ChainSwapData>, Error> {
        let claim_address = claim_address.to_string();
        let refund_address = refund_address.to_string();
        swaps
            .iter()
            .filter(|e| matches!(e.swap_type, SwapRestoreType::Chain))
            .filter(|e| e.to == "BTC" && e.from == "L-BTC")
            .map(|e| {
                convert_swap_restore_response_to_chain_swap_data(
                    e,
                    &self.mnemonic,
                    &claim_address,
                    &refund_address,
                )
            })
            .collect()
    }
}

async fn fetch_lockup_txid(api: &BoltzApiClientV2, swap_id: &str) -> Option<String> {
    match api.get_chain_txs(swap_id).await {
        Ok(txs) => match txs.user_lock {
            Some(lockup) => Some(lockup.transaction.id),
            None => {
                log::warn!(
                    "failed to fetch chain lockup txid for swap {}: server_lock is missing",
                    swap_id
                );
                None
            }
        },
        Err(err) => {
            log::warn!(
                "failed to fetch chain lockup txid for swap {}: {}",
                swap_id,
                err
            );
            None
        }
    }
}

/// Convert a swap restore response from Boltz API to a ChainSwapData.
///
/// Note: This function uses `&str` for addresses instead of typed `bitcoin::Address` or
/// `elements::Address` because it handles both swap directions (BTC→LBTC and LBTC→BTC).
/// The address types are swapped depending on the direction, so type safety is enforced
/// at the public API level via `restorable_btc_to_lbtc_swaps` and `restorable_lbtc_to_btc_swaps`.
pub(crate) fn convert_swap_restore_response_to_chain_swap_data(
    e: &SwapRestoreResponse,
    mnemonic: &Mnemonic,
    claim_address: &str,
    refund_address: &str,
) -> Result<ChainSwapData, Error> {
    // Only handle chain swaps
    match e.swap_type {
        SwapRestoreType::Chain => {}
        _ => {
            return Err(Error::SwapRestoration(format!(
                "Only chain swaps are supported for restoration, got: {:?}",
                e.swap_type
            )))
        }
    }

    // Extract claim details (required for chain swaps - this is boltz's lockup that we claim from)
    let claim_details = e.claim_details.as_ref().ok_or_else(|| {
        Error::SwapRestoration(format!("Chain swap {} is missing claim_details", e.id))
    })?;

    // Extract refund details (required for chain swaps - this is our lockup)
    let refund_details = e.refund_details.as_ref().ok_or_else(|| {
        Error::SwapRestoration(format!("Chain swap {} is missing refund_details", e.id))
    })?;

    // Derive the keypairs from the mnemonic at the key indices
    let claim_keys = derive_keypair(claim_details.key_index, mnemonic)?;
    let refund_keys = derive_keypair(refund_details.key_index, mnemonic)?;

    // Derive preimage from claim keys (deterministic)
    let preimage = preimage_from_keypair(&claim_keys);

    // Parse chains from the response
    let from_chain = chain_from_str(&e.from)?;
    let to_chain = chain_from_str(&e.to)?;

    // Parse server public keys
    let claim_server_pubkey_bitcoin = BitcoinPublicKey::from_str(&claim_details.server_public_key)
        .map_err(|err| {
            Error::SwapRestoration(format!("Failed to parse claim server public key: {err}"))
        })?;
    let claim_server_pubkey = PublicKey {
        inner: claim_server_pubkey_bitcoin.inner,
        compressed: claim_server_pubkey_bitcoin.compressed,
    };

    let refund_server_pubkey_bitcoin =
        BitcoinPublicKey::from_str(&refund_details.server_public_key).map_err(|err| {
            Error::SwapRestoration(format!("Failed to parse refund server public key: {err}"))
        })?;
    let refund_server_pubkey = PublicKey {
        inner: refund_server_pubkey_bitcoin.inner,
        compressed: refund_server_pubkey_bitcoin.compressed,
    };

    // Convert ClaimDetails to ChainSwapDetails for claim side
    let chain_claim_details = ChainSwapDetails {
        swap_tree: claim_details.tree.clone(),
        lockup_address: claim_details.lockup_address.clone(),
        server_public_key: claim_server_pubkey,
        timeout_block_height: claim_details.timeout_block_height,
        amount: claim_details.amount.unwrap_or(0),
        blinding_key: claim_details.blinding_key.clone(),
        refund_address: None,
        claim_address: None,
        bip21: None,
    };

    // Convert RefundDetails to ChainSwapDetails for lockup side
    // Note: RefundDetails doesn't have an amount field in the boltz_client type,
    // so we use 0 as default (the actual amount was already sent to the lockup address)
    let chain_lockup_details = ChainSwapDetails {
        swap_tree: refund_details.tree.clone(),
        lockup_address: refund_details.lockup_address.clone(),
        server_public_key: refund_server_pubkey,
        timeout_block_height: refund_details.timeout_block_height,
        amount: 0, // Not available in RefundDetails type
        blinding_key: refund_details.blinding_key.clone(),
        refund_address: None,
        claim_address: None,
        bip21: None,
    };

    // Reconstruct CreateChainResponse
    let create_chain_response = CreateChainResponse {
        id: e.id.clone(),
        claim_details: chain_claim_details,
        lockup_details: chain_lockup_details,
    };

    let lockup_address = create_chain_response.lockup_details.lockup_address.clone();
    let expected_lockup_amount = create_chain_response.lockup_details.amount;

    // Parse the status to SwapState
    let last_state = e.status.parse::<SwapState>().map_err(|err| {
        Error::SwapRestoration(format!(
            "Failed to parse status '{}' as SwapState: {}",
            e.status, err
        ))
    })?;

    Ok(ChainSwapData {
        last_state,
        swap_type: SwapType::Chain,
        fee: None, // Fee information not available in restore response
        boltz_fee: None,
        claim_fee: None, // Not available in restore response, will use fallback fee rate
        create_chain_response,
        claim_keys,
        refund_keys,
        preimage,
        lockup_address,
        expected_lockup_amount,
        claim_address: claim_address.to_string(),
        refund_address: refund_address.to_string(),
        claim_key_index: claim_details.key_index,
        refund_key_index: refund_details.key_index,
        mnemonic_identifier: mnemonic_identifier(mnemonic)?,
        from_chain,
        to_chain,
        random_preimage: false, // when trying to restore from boltz only deterministic preimage are supported
        claim_txid: None, // claim_details.transaction is the lockup tx, boltz don't track claim tx
        lockup_txid: refund_details.transaction.as_ref().map(|e| e.id.clone()),
        refund_txid: None, // boltz don't track refund tx
    })
}

impl LockupResponse {
    pub fn claim_txid(&self) -> Option<&str> {
        self.data.claim_txid.as_deref()
    }

    /// The txid of the user lockup transaction of the swap
    pub fn lockup_txid(&self) -> Option<&str> {
        self.data.lockup_txid.as_deref()
    }

    /// The txid of the refund transaction of the swap
    pub fn refund_txid(&self) -> Option<&str> {
        self.data.refund_txid.as_deref()
    }

    /// Optionally set the lockup transaction txid.
    ///
    /// This is useful for apps that create and broadcast the lockup transaction and want to
    /// immediately store the txid before Boltz websocket updates arrive. Doing so can prevent a
    /// race where a very fast retry flow might create and send the lockup transaction twice.
    pub fn set_lockup_txid(&mut self, txid: String) -> Result<(), Error> {
        self.data.lockup_txid = Some(txid);
        self.persist()?;
        Ok(())
    }

    async fn next_status(&mut self) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id().to_string();
        next_status(&mut self.rx, self.timeout_advance, &swap_id, self.polling).await
    }

    pub fn lockup_address(&self) -> &str {
        &self.data.lockup_address
    }

    pub fn claim_address(&self) -> &str {
        &self.data.claim_address
    }

    pub fn expected_amount(&self) -> u64 {
        self.data.expected_lockup_amount
    }

    /// The BIP21 URI for the lockup address, if provided by Boltz
    pub fn uri(&self) -> Option<&str> {
        self.data
            .create_chain_response
            .lockup_details
            .bip21
            .as_deref()
    }

    pub fn chain_from(&self) -> Chain {
        self.data.from_chain
    }

    pub fn chain_to(&self) -> Chain {
        self.data.to_chain
    }

    /// The fee of the swap provider and the network fee
    ///
    /// It is equal to the amount requested minus the amount sent to the claim address.
    pub fn fee(&self) -> Option<u64> {
        self.data.fee
    }

    /// The fee of the swap provider
    ///
    /// It is equal to the swap amount multiplied by the boltz fee rate.
    pub fn boltz_fee(&self) -> Option<u64> {
        self.data.boltz_fee
    }

    async fn build_and_broadcast_refund(&mut self) -> Result<(), Error> {
        sleep(WAIT_TIME).await;
        let tx = self
            .lockup_script
            .construct_refund(SwapTransactionParams {
                keys: self.data.refund_keys,
                output_address: self.data.refund_address.clone(),
                fee: Fee::Relative(1.0),
                swap_id: self.swap_id().to_string(),
                chain_client: &self.chain_client,
                boltz_client: &self.api,
                options: None,
            })
            .await?;
        let txid = broadcast_tx_with_retry(&self.chain_client, &tx).await?;
        self.data.refund_txid = Some(txid.clone());
        log::info!("Refund transaction broadcasted: {txid}");
        Ok(())
    }

    pub async fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let update = self.next_status().await?;
        let update_status = update.swap_state()?;

        let flow = match update_status {
            SwapState::SwapCreated => Ok(ControlFlow::Continue(update)),
            SwapState::TransactionMempool => {
                let lockup_txid = update.transaction.as_ref().map(|tx| tx.id.clone());
                log::info!("User lockup in mempool {lockup_txid:?}");
                self.data.lockup_txid = lockup_txid;
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionConfirmed => {
                let lockup_txid = update.transaction.as_ref().map(|tx| tx.id.clone());
                log::info!("User lockup confirmed {lockup_txid:?}, waiting for server lockup");
                if self.data.lockup_txid.is_none() {
                    self.data.lockup_txid = lockup_txid;
                }
                Ok(ControlFlow::Continue(update))
            }
            SwapState::ServerTransactionMempool => {
                log::info!("Server lockup in mempool");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::ServerTransactionConfirmed => {
                // Parse the server's lockup transaction from the status update if available.
                // This avoids waiting for the transaction to propagate to the chain client's mempool,
                // significantly improving claim speed.
                let lockup_tx = if let Some(tx_info) = &update.transaction {
                    match self.claim_script.parse_lockup_transaction(tx_info).await {
                        Ok(tx) => {
                            log::debug!("Parsed server lockup tx from status update");
                            Some(tx)
                        }
                        Err(e) => {
                            log::warn!("Failed to parse server lockup tx from status update: {e}, will fetch from chain client");
                            None
                        }
                    }
                } else {
                    log::debug!(
                        "No transaction info in status update, will fetch from chain client"
                    );
                    None
                };

                // If we don't have the lockup tx, fall back to waiting for propagation
                if lockup_tx.is_none() {
                    sleep(WAIT_TIME).await;
                }

                // Attempt cooperative (key path) claim
                self.build_and_broadcast_claim(true, lockup_tx).await?;
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionClaimed => {
                // Boltz has claimed the user's lockup, but we still need to claim from Boltz's lockup
                if self.data.lockup_txid.is_none() {
                    log::warn!("transaction.claimed but lockup_txid is not set, fetching it");
                    self.data.lockup_txid =
                        fetch_lockup_txid(self.api.as_ref(), self.swap_id()).await;
                }

                // Check if we've already claimed our funds
                if self.data.claim_txid.is_some() {
                    log::info!("User already claimed their funds, swap completed successfully");
                    Ok(ControlFlow::Break(true))
                } else {
                    // Boltz has already claimed, so we can't use cooperative claiming
                    // We need to claim via script path instead
                    log::warn!(
                        "Boltz has already claimed (transaction.claimed), attempting non-cooperative claim via script path"
                    );

                    // Attempt non-cooperative (script path) claim
                    self.build_and_broadcast_claim(false, None).await?;

                    Ok(ControlFlow::Break(true))
                }
            }
            SwapState::TransactionLockupFailed => {
                log::warn!("User lockup failed, performing refund");

                self.build_and_broadcast_refund().await?;
                Ok(ControlFlow::Break(true))
            }
            SwapState::TransactionFailed => {
                log::warn!(
                    "Boltz failed server lockup, performing refund. reason: {:?}, details: {:?}",
                    update.failure_reason,
                    update.failure_details
                );
                self.build_and_broadcast_refund().await?;
                Ok(ControlFlow::Break(true))
            }
            SwapState::TransactionRefunded => {
                log::info!("Boltz refunded their stash, we do the same with ours");

                self.build_and_broadcast_refund().await?;
                Ok(ControlFlow::Break(true))
            }
            SwapState::SwapExpired => {
                log::warn!("Chain swap expired");
                // TODO: non-cooperative refund if possible
                Ok(ControlFlow::Break(false))
            }
            ref e => Err(Error::UnexpectedUpdate {
                swap_id: self.swap_id().to_string(),
                status: e.to_string(),
                last_state: self.data.last_state,
            }),
        };

        let is_completed = matches!(flow.as_ref(), Ok(ControlFlow::Break(_)));

        if is_completed {
            // if the swap is terminated, but the caller call advance() again we don't
            // want to error for timeout (it will trigger NoBoltzUpdate)
            self.polling = true;
        }

        self.data.last_state = update_status;

        // Persist state changes
        if flow.is_ok() {
            if is_completed {
                // Final persist and move to completed list
                self.persist()?;
                self.move_to_completed()?;
            } else {
                // Persist intermediate state
                self.persist()?;
            }
        }

        flow
    }

    /// Construct and broadcast claim transaction
    ///
    /// # Arguments
    /// * `cooperative` - If true, attempts cooperative (key path) claim with Boltz's signature.
    ///                   If false, uses script path claim (required when Boltz already claimed).
    /// * `lockup_tx` - Optional pre-fetched lockup transaction. If None, will be fetched from chain.
    async fn build_and_broadcast_claim(
        &mut self,
        cooperative: bool,
        lockup_tx: Option<BtcLikeTransaction>,
    ) -> Result<String, Error> {
        log::info!(
            "Claiming on {} chain (cooperative: {})",
            self.chain_to(),
            cooperative
        );

        // Build options with or without lockup_tx
        let options = match lockup_tx {
            Some(tx) => TransactionOptions::default()
                .with_chain_claim(self.data.refund_keys, self.lockup_script.clone())
                .with_cooperative(cooperative)
                .with_lockup_tx(tx),
            None => TransactionOptions::default()
                .with_chain_claim(self.data.refund_keys, self.lockup_script.clone())
                .with_cooperative(cooperative),
        };

        // Use the claim fee from Boltz API to match the quoted amount exactly.
        // For Liquid claims (BTC→L-BTC), add LIQUID_UNCOOPERATIVE_EXTRA as buffer.
        // Fall back to Fee::Relative if claim_fee is not available (e.g., restored swaps).
        let fee = match self.data.claim_fee {
            Some(claim_fee) => {
                // Add extra for Liquid claims only (to_chain is Liquid)
                let extra = if matches!(self.data.to_chain, Chain::Liquid(_)) {
                    LIQUID_UNCOOPERATIVE_EXTRA
                } else {
                    0
                };
                Fee::Absolute(claim_fee + extra)
            }
            None => Fee::Relative(1.0),
        };

        let tx = self
            .claim_script
            .construct_claim(
                &self.data.preimage,
                SwapTransactionParams {
                    keys: self.data.claim_keys,
                    output_address: self.data.claim_address.clone(),
                    fee,
                    swap_id: self.swap_id().to_string(),
                    chain_client: &self.chain_client,
                    boltz_client: &self.api,
                    options: Some(options),
                },
            )
            .await?;

        #[cfg(debug_assertions)]
        {
            // Simulate app crash AFTER construct_claim (preimage revealed to Boltz)
            // but BEFORE broadcast_tx (user never gets funds)
            // Only compiled in debug builds, excluded from release builds
            if std::env::var("LWKBOLTZ_TEST_CRASH_AFTER_CONSTRUCT").is_ok() {
                log::warn!("TEST: Simulating crash AFTER construct_claim, BEFORE broadcast");

                return Err(Error::Generic(
                    "Simulated crash after construct_claim for testing".to_string(),
                ));
            }
        }

        let txid = broadcast_tx_with_retry(&self.chain_client, &tx).await?;
        self.data.claim_txid = Some(txid.clone());
        log::info!("Claim transaction broadcasted successfully: {}", txid);
        Ok(txid)
    }

    pub async fn complete(mut self) -> Result<bool, Error> {
        loop {
            match self.advance().await? {
                ControlFlow::Continue(update) => {
                    log::info!("Received update: {}", update.status);
                }
                ControlFlow::Break(success) => {
                    return Ok(success);
                }
            }
        }
    }
}
