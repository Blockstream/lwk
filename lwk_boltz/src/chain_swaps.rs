use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bip39::Mnemonic;
use boltz_client::boltz::{
    BoltzApiClientV2, ChainSwapDetails, ChainSwapStates, CreateChainRequest, CreateChainResponse,
    Side, SwapRestoreResponse, SwapRestoreType, SwapStatus, Webhook,
};
use boltz_client::fees::Fee;
use boltz_client::network::Chain;
use boltz_client::swaps::{ChainClient, SwapScript, SwapTransactionParams, TransactionOptions};
use boltz_client::util::sleep;
use boltz_client::PublicKey;
use lwk_wollet::bitcoin::PublicKey as BitcoinPublicKey;
use lwk_wollet::elements;
use lwk_wollet::elements::bitcoin;

use crate::chain_data::{chain_from_str, to_chain_data, ChainSwapData, ChainSwapDataSerializable};
use crate::error::Error;
use crate::swap_state::SwapStateTrait;
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
        let fee = amount.saturating_sub(expected_lockup_amount);

        Ok(LockupResponse {
            data: ChainSwapData {
                last_state,
                swap_type: SwapType::Chain,
                fee: Some(fee),
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
            },
            lockup_script,
            claim_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
            polling: self.polling,
            timeout_advance: self.timeout_advance,
        })
    }

    pub async fn restore_lockup(
        &self,
        data: ChainSwapDataSerializable,
    ) -> Result<LockupResponse, Error> {
        let data = to_chain_data(data, &self.mnemonic)?;
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

        Ok(LockupResponse {
            data,
            lockup_script,
            claim_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
            polling: self.polling,
            timeout_advance: self.timeout_advance,
        })
    }

    /// From the swaps returned by the boltz api via [`BoltzSession::swap_restore`]:
    ///
    /// - filter the chain swaps that can be restored
    /// - Add the private information from the session needed to restore the swap
    ///
    /// The claim and refund addresses don't need to be the same used when creating the swap.
    pub async fn restorable_chain_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &str,
        refund_address: &str,
    ) -> Result<Vec<ChainSwapData>, Error> {
        swaps
            .iter()
            .filter(|e| matches!(e.swap_type, SwapRestoreType::Chain))
            .filter(|e| e.status != "swap.expired" && e.status != "transaction.claimed")
            .map(|e| {
                convert_swap_restore_response_to_chain_swap_data(
                    e,
                    &self.mnemonic,
                    claim_address,
                    refund_address,
                )
            })
            .collect()
    }
}

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
            Error::SwapRestoration(format!("Failed to parse claim server public key: {}", err))
        })?;
    let claim_server_pubkey = PublicKey {
        inner: claim_server_pubkey_bitcoin.inner,
        compressed: claim_server_pubkey_bitcoin.compressed,
    };

    let refund_server_pubkey_bitcoin =
        BitcoinPublicKey::from_str(&refund_details.server_public_key).map_err(|err| {
            Error::SwapRestoration(format!("Failed to parse refund server public key: {}", err))
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
        timeout_block_height: refund_details.timeout_block_height as u32,
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
    })
}

impl LockupResponse {
    async fn next_status(&mut self) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id();
        next_status(&mut self.rx, self.timeout_advance, &swap_id, self.polling).await
    }

    pub fn swap_id(&self) -> String {
        self.data.create_chain_response.id.clone()
    }

    pub fn lockup_address(&self) -> &str {
        &self.data.lockup_address
    }

    pub fn expected_amount(&self) -> u64 {
        self.data.expected_lockup_amount
    }

    pub fn chain_from(&self) -> Chain {
        self.data.from_chain
    }

    pub fn chain_to(&self) -> Chain {
        self.data.to_chain
    }

    pub async fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let update = self.next_status().await?;
        let update_status = update.swap_state()?;

        let flow = match update_status {
            SwapState::SwapCreated => Ok(ControlFlow::Continue(update)),
            SwapState::TransactionMempool => {
                log::info!("User lockup in mempool");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionConfirmed => {
                log::info!("User lockup confirmed, waiting for server lockup");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::ServerTransactionMempool => {
                log::info!("Server lockup in mempool");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::ServerTransactionConfirmed => {
                log::info!(
                    "Server lockup confirmed, claiming on {} chain",
                    self.chain_to()
                );
                sleep(WAIT_TIME).await; //TODO can we do better?
                let tx = self
                    .claim_script
                    .construct_claim(
                        &self.data.preimage,
                        SwapTransactionParams {
                            keys: self.data.claim_keys,
                            output_address: self.data.claim_address.clone(),
                            fee: Fee::Relative(1.0),
                            swap_id: self.swap_id(),
                            chain_client: &self.chain_client,
                            boltz_client: &self.api,
                            options: Some(TransactionOptions::default().with_chain_claim(
                                self.data.refund_keys,
                                self.lockup_script.clone(),
                            )),
                        },
                    )
                    .await?;
                broadcast_tx_with_retry(&self.chain_client, &tx).await?;
                log::info!("Claim transaction broadcasted successfully");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionClaimed => {
                log::info!("Swap claimed successfully");
                Ok(ControlFlow::Break(true))
            }
            SwapState::TransactionLockupFailed => {
                log::warn!("User lockup failed, performing refund");
                sleep(WAIT_TIME).await;
                let tx = self
                    .lockup_script
                    .construct_refund(SwapTransactionParams {
                        keys: self.data.refund_keys,
                        output_address: self.data.refund_address.clone(),
                        fee: Fee::Relative(1.0),
                        swap_id: self.swap_id(),
                        chain_client: &self.chain_client,
                        boltz_client: &self.api,
                        options: None,
                    })
                    .await?;
                let txid = broadcast_tx_with_retry(&self.chain_client, &tx).await?;
                log::info!("Refund transaction broadcasted: {txid}");
                Ok(ControlFlow::Break(true))
            }
            SwapState::SwapExpired => {
                log::warn!("Chain swap expired");
                // TODO: non-cooperative refund if possible
                Ok(ControlFlow::Break(false))
            }
            ref e => Err(Error::UnexpectedUpdate {
                swap_id: self.swap_id(),
                status: e.to_string(),
                last_state: self.data.last_state,
            }),
        };

        if let Ok(ControlFlow::Break(_)) = flow.as_ref() {
            // if the swap is terminated, but the caller call advance() again we don't
            // want to error for timeout (it will trigger NoBoltzUpdate)
            self.polling = true;
        }

        self.data.last_state = update_status;
        flow
    }

    pub fn serialize(&self) -> Result<String, Error> {
        let s: ChainSwapDataSerializable = self.data.clone().into();
        Ok(serde_json::to_string(&s)?)
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
