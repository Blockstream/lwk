use std::fmt;
use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bip39::Mnemonic;
use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::CreateReverseRequest;
use boltz_client::boltz::RevSwapStates;
use boltz_client::boltz::SwapRestoreResponse;
use boltz_client::boltz::SwapRestoreType;
use boltz_client::boltz::SwapStatus;
use boltz_client::boltz::Webhook;
use boltz_client::boltz::{ClaimDetails, CreateReverseResponse};
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::find_magic_routing_hint;
use boltz_client::swaps::magic_routing::sign_address;
use boltz_client::swaps::ChainClient;
use boltz_client::swaps::SwapScript;
use boltz_client::swaps::SwapTransactionParams;
use boltz_client::swaps::TransactionOptions;
use boltz_client::Bolt11Invoice;
use boltz_client::PublicKey;
use lwk_wollet::elements;

use crate::derive_keypair;
use crate::error::Error;
use crate::invoice_data::InvoiceData;
use crate::invoice_data::InvoiceDataSerializable;
use crate::mnemonic_identifier;
use crate::preimage_from_keypair;
use crate::swap_state::SwapStateTrait;
use crate::to_invoice_data;
use crate::DynStore;
use crate::SwapPersistence;
use crate::SwapType;
use crate::LIQUID_UNCOOPERATIVE_EXTRA;
use crate::{broadcast_tx_with_retry, next_status, BoltzSession, SwapState};

pub struct InvoiceResponse {
    pub data: InvoiceData,

    // unserializable fields
    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    swap_script: SwapScript,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    polling: bool,
    timeout_advance: Duration,
    store: Option<Arc<dyn DynStore>>,
    store_prefix: String,
}

impl fmt::Debug for InvoiceResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InvoiceResponse {{ data: {:?}, rx: {:?}, swap_script: {:?}, api: {:?}, polling: {:?}, timeout_advance: {:?}, store: {:?} }}", self.data, self.rx, self.swap_script, self.api, self.polling, self.timeout_advance, self.store)
    }
}

impl SwapPersistence for InvoiceResponse {
    fn serialize(&self) -> Result<String, Error> {
        let x = InvoiceDataSerializable::from(self.data.clone());
        Ok(serde_json::to_string(&x)?)
    }

    fn swap_id(&self) -> &str {
        &self.data.create_reverse_response.id
    }

    fn store(&self) -> Option<&Arc<dyn DynStore>> {
        self.store.as_ref()
    }

    fn store_prefix(&self) -> &str {
        &self.store_prefix
    }
}

impl BoltzSession {
    pub async fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &elements::Address,
        webhook: Option<Webhook<RevSwapStates>>,
    ) -> Result<InvoiceResponse, Error> {
        let chain = self.chain();
        let (key_index, our_keys) = self.derive_next_keypair()?;
        let preimage = self.preimage(&our_keys);

        let claim_public_key = PublicKey {
            compressed: true,
            inner: our_keys.public_key(),
        };
        let webhook_str = format!("{webhook:?}");

        let addrs_sig = sign_address(&claim_address.to_string(), &our_keys)?;
        let create_reverse_req = CreateReverseRequest {
            from: "BTC".to_string(),
            to: chain.to_string(),
            invoice: None,
            invoice_amount: Some(amount),
            preimage_hash: Some(preimage.sha256),
            description,
            description_hash: None,
            address_signature: Some(addrs_sig.to_string()),
            address: Some(claim_address.to_string()),
            claim_public_key,
            referral_id: self.referral_id.clone(),
            webhook,
        };

        let reverse_resp = self.api.post_reverse_req(create_reverse_req).await?;
        let invoice_str = reverse_resp
            .invoice
            .as_ref()
            .ok_or(Error::MissingInvoiceInResponse(reverse_resp.id.clone()))?
            .clone();
        let invoice = Bolt11Invoice::from_str(&invoice_str)?;
        let fee = amount.checked_sub(reverse_resp.onchain_amount).ok_or(
            Error::ExpectedAmountLowerThanInvoice(amount, reverse_resp.id.clone()),
        )?;

        let (boltz_fee, claim_fee) = {
            let swap_info = self.swap_info.lock().await;
            match swap_info.reverse_pairs.get_btc_to_lbtc_pair() {
                Some(pair) => (
                    Some(pair.fees.boltz(amount)),
                    Some(pair.fees.claim_estimate()),
                ),
                None => (None, None),
            }
        };

        // Sanity check: Boltz-created invoices should always have the magic routing hint.
        // We use find_magic_routing_hint (local parsing) instead of check_for_mrh
        // to avoid an unnecessary network call to get_mrh_bip21.
        let _ = find_magic_routing_hint(&invoice_str)?.ok_or(
            Error::InvoiceWithoutMagicRoutingHint(reverse_resp.id.clone()),
        )?;

        log::debug!("Got Reverse swap response: {reverse_resp:?}");

        let swap_script =
            SwapScript::reverse_from_swap_resp(chain, &reverse_resp, claim_public_key)?;
        let swap_id = reverse_resp.id.clone();
        log::info!("subscribing to swap: {swap_id} webhook:{webhook_str}");
        self.ws.subscribe_swap(&swap_id).await?;
        let mut rx = self.ws.updates();

        let update = next_status(&mut rx, self.timeout, &swap_id, false).await?;
        let last_state = update.swap_state()?;
        log::debug!("Waiting for Invoice to be paid: {}", &invoice);

        let store = self.clone_store();
        let store_prefix = self.clone_store_prefix();
        let response = InvoiceResponse {
            polling: self.polling,
            timeout_advance: self.timeout_advance,
            data: InvoiceData {
                last_state,
                swap_type: SwapType::Reverse,
                fee: Some(fee),
                boltz_fee,
                claim_fee,
                claim_txid: None,
                create_reverse_response: reverse_resp.clone(),
                our_keys,
                preimage,
                claim_address: claim_address.clone(),
                key_index,
                mnemonic_identifier: mnemonic_identifier(&self.mnemonic)?,
                claim_broadcasted: false,
                random_preimage: self.random_preimages,
            },
            rx,
            swap_script,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
            store,
            store_prefix,
        };

        // Persist swap data and add to pending list
        response.persist_and_add_to_pending()?;

        Ok(response)
    }

    pub async fn restore_invoice(
        &self,
        data: InvoiceDataSerializable,
    ) -> Result<InvoiceResponse, Error> {
        let data = to_invoice_data(data, &self.mnemonic)?;
        let p = data.our_keys.public_key();
        let swap_script = SwapScript::reverse_from_swap_resp(
            self.chain(),
            &data.create_reverse_response,
            PublicKey {
                inner: p,
                compressed: true,
            },
        )?;
        let swap_id = data.create_reverse_response.id.clone();
        let rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;

        let response = InvoiceResponse {
            polling: self.polling,
            timeout_advance: self.timeout_advance,
            data,
            rx,
            swap_script,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
            store: self.clone_store(),
            store_prefix: self.clone_store_prefix(),
        };

        // If the swap was already in a terminal state, move it to completed
        if response.data.last_state.is_terminal() {
            response.move_to_completed()?;
        }

        Ok(response)
    }

    /// From the swaps returned by the boltz api via [`BoltzSession::swap_restore`]:
    ///
    /// - filter the reverse swaps that can be restored
    /// - Add the private information from the session needed to restore the swap
    ///
    /// The claim address doesn't need to be the same used when creating the swap.
    pub async fn restorable_reverse_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &elements::Address,
    ) -> Result<Vec<InvoiceData>, Error> {
        swaps
            .iter()
            .filter(|e| matches!(e.swap_type, SwapRestoreType::Reverse))
            .filter(|e| {
                e.status != "swap.expired"
                    && e.status != "invoice.settled"
                    && e.status != "swap.created"
            })
            .map(|e| {
                convert_swap_restore_response_to_invoice_data(e, &self.mnemonic, claim_address)
            })
            .collect()
    }
}

pub(crate) fn convert_swap_restore_response_to_invoice_data(
    e: &boltz_client::boltz::SwapRestoreResponse,
    mnemonic: &Mnemonic,
    claim_address: &elements::Address,
) -> Result<InvoiceData, Error> {
    // Only handle reverse swaps for now
    match e.swap_type {
        SwapRestoreType::Reverse => {}
        _ => {
            return Err(Error::SwapRestoration(format!(
                "Only reverse swaps are supported for restoration, got: {:?}",
                e.swap_type
            )))
        }
    }

    // Extract claim details (required for reverse swaps)
    let claim_details: &ClaimDetails = e.claim_details.as_ref().ok_or_else(|| {
        Error::SwapRestoration(format!("Reverse swap {} is missing claim_details", e.id))
    })?;

    // Derive the keypair from the mnemonic at the key_index
    let our_keys = derive_keypair(claim_details.key_index, mnemonic)?;

    let preimage = preimage_from_keypair(&our_keys);

    // Parse the server public key
    let refund_public_key_bitcoin = lwk_wollet::bitcoin::PublicKey::from_str(
        &claim_details.server_public_key,
    )
    .map_err(|e| Error::SwapRestoration(format!("Failed to parse server public key: {e}")))?;
    let refund_public_key = PublicKey {
        inner: refund_public_key_bitcoin.inner,
        compressed: refund_public_key_bitcoin.compressed,
    };

    // Reconstruct CreateReverseResponse from ClaimDetails
    let create_reverse_response = CreateReverseResponse {
        id: e.id.clone(),
        invoice: None, // Not available in restore response
        swap_tree: claim_details.tree.clone(),
        lockup_address: claim_details.lockup_address.clone(),
        refund_public_key,
        timeout_block_height: claim_details.timeout_block_height,
        onchain_amount: claim_details.amount.unwrap_or(0), // TODO, not sure how to handle this better
        blinding_key: claim_details.blinding_key.clone(),
    };

    // Parse the status to SwapState
    let last_state = e.status.parse::<SwapState>().map_err(|err| {
        Error::SwapRestoration(format!(
            "Failed to parse status '{}' as SwapState: {err}",
            e.status
        ))
    })?;

    Ok(InvoiceData {
        last_state,
        swap_type: SwapType::Reverse,
        fee: None, // Fee information not available in restore response
        boltz_fee: None,
        claim_fee: None, // Not available in restore response, will use fallback fee rate
        claim_txid: None,
        create_reverse_response,
        our_keys,
        preimage,
        claim_address: claim_address.clone(),
        key_index: claim_details.key_index,
        mnemonic_identifier: mnemonic_identifier(mnemonic)?,
        claim_broadcasted: false,
        random_preimage: false, // when trying to restore from boltz only deterministic preimage are supported
    })
}

impl InvoiceResponse {
    async fn next_status(&mut self) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id().to_string();
        next_status(&mut self.rx, self.timeout_advance, &swap_id, self.polling).await
    }

    async fn handle_claim_transaction_if_necessary(
        &mut self,
        update: SwapStatus,
    ) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        if self.data.claim_broadcasted {
            return Ok(ControlFlow::Continue(update));
        }

        log::info!("transaction.mempool/confirmed Boltz broadcasted funding tx");

        // Parse the lockup transaction from the status update if available.
        // This avoids waiting for the transaction to propagate to the chain client's mempool,
        // significantly improving claim speed.
        let lockup_tx = if let Some(tx_info) = &update.transaction {
            match self.swap_script.parse_lockup_transaction(tx_info).await {
                Ok(tx) => {
                    log::debug!("Parsed lockup tx from status update");
                    Some(tx)
                }
                Err(e) => {
                    log::warn!("Failed to parse lockup tx from status update: {e}, will fetch from chain client");
                    None
                }
            }
        } else {
            log::debug!("No transaction info in status update, will fetch from chain client");
            None
        };

        // Build options with lockup_tx if available for faster claiming
        let options = match lockup_tx {
            Some(tx) => TransactionOptions::default()
                .with_cooperative(true)
                .with_lockup_tx(tx),
            None => TransactionOptions::default().with_cooperative(true),
        };

        // Use the claim fee from Boltz API to match the quoted amount exactly.
        // Add LIQUID_UNCOOPERATIVE_EXTRA as buffer for script-path claims.
        // Fall back to Fee::Relative if claim_fee is not available (e.g., restored swaps).
        let fee = match self.data.claim_fee {
            Some(claim_fee) => Fee::Absolute(claim_fee + LIQUID_UNCOOPERATIVE_EXTRA),
            None => Fee::Relative(0.12),
        };

        let tx = self
            .swap_script
            .construct_claim(
                &self.data.preimage,
                SwapTransactionParams {
                    keys: self.data.our_keys,
                    output_address: self.data.claim_address.to_string(),
                    fee,
                    swap_id: self.swap_id().to_string(),
                    options: Some(options),
                    chain_client: &self.chain_client,
                    boltz_client: &self.api,
                },
            )
            .await?;

        let txid = broadcast_tx_with_retry(&self.chain_client, &tx).await?;
        self.data.claim_txid = Some(txid);
        self.data.claim_broadcasted = true;

        log::info!("Successfully broadcasted claim tx!");
        log::debug!("Claim Tx {tx:?}");
        Ok(ControlFlow::Continue(update))
    }

    pub fn bolt11_invoice(&self) -> Bolt11Invoice {
        Bolt11Invoice::from_str(self.data.create_reverse_response.invoice.as_ref().expect(
            "Invoice must be present or we would have errored on the BoltzSession::invoice",
        ))
        .expect("Invoice must be parsable or we would have errored on the BoltzSession::invoice")
    }

    /// The fee of the swap provider and the network fee
    ///
    /// It is equal to the amount of the invoice minus the amount of the onchain transaction.
    pub fn fee(&self) -> Option<u64> {
        self.data.fee
    }

    /// The fee of the swap provider
    ///
    /// It is equal to the invoice amount multiplied by the boltz fee rate.
    /// For example for receiving an invoice of 10000 satoshi with a 0.25% rate would be 25 satoshi.
    pub fn boltz_fee(&self) -> Option<u64> {
        self.data.boltz_fee
    }

    /// The txid of the claim transaction of the swap
    pub fn claim_txid(&self) -> Option<&str> {
        self.data.claim_txid.as_deref()
    }

    /// The claim transaction fee estimate from Boltz API (in satoshis)
    pub fn claim_fee(&self) -> Option<u64> {
        self.data.claim_fee
    }

    /// The onchain amount that Boltz locks up for the swap
    pub fn onchain_amount(&self) -> u64 {
        self.data.create_reverse_response.onchain_amount
    }

    pub async fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let update = self.next_status().await?;
        let update_status = update.swap_state()?;

        let flow = match update_status {
            SwapState::SwapCreated => Ok(ControlFlow::Continue(update)),
            SwapState::TransactionDirect => {
                log::info!("transaction.direct Payer used magic routing hint");
                Ok(ControlFlow::Break(true))
            }
            SwapState::TransactionMempool => {
                log::info!("transaction.mempool Boltz funding tx");
                self.handle_claim_transaction_if_necessary(update).await
            }
            SwapState::TransactionConfirmed => {
                log::info!("transaction.confirmed Boltz funding tx");
                self.handle_claim_transaction_if_necessary(update).await
            }
            SwapState::InvoiceSettled => {
                log::info!("invoice.settled Reverse Swap Successful!");
                Ok(ControlFlow::Break(true))
            }
            SwapState::SwapExpired => {
                log::warn!("swap.expired Boltz swap expired");
                Ok(ControlFlow::Break(false))
            }
            SwapState::InvoiceExpired => {
                log::warn!("invoice.expired Boltz invoice expired");
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

    pub async fn complete_pay(mut self) -> Result<bool, Error> {
        loop {
            match self.advance().await? {
                ControlFlow::Continue(update) => {
                    log::info!("Received update. status:{}", update.status);
                }
                ControlFlow::Break(e) => {
                    break Ok(e);
                }
            }
        }
    }
}
