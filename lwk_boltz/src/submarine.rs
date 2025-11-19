use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bip39::Mnemonic;
use boltz_client::boltz::{
    BoltzApiClientV2, CreateSubmarineRequest, CreateSubmarineResponse, RefundDetails,
    SubSwapStates, SwapRestoreResponse, SwapRestoreType, SwapStatus, Webhook,
};
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::{ChainClient, SwapScript, SwapTransactionParams};
use boltz_client::util::sleep;
use boltz_client::PublicKey;
use lwk_wollet::bitcoin::{Denomination, PublicKey as BitcoinPublicKey};
use lwk_wollet::elements;

use crate::error::Error;
use crate::prepare_pay_data::{to_prepare_pay_data, PreparePayData, PreparePayDataSerializable};
use crate::swap_state::SwapStateTrait;
use crate::{
    broadcast_tx_with_retry, mnemonic_identifier, next_status, BoltzSession, LightningPayment,
    SwapState, SwapType, WAIT_TIME,
};

pub struct PreparePayResponse {
    pub data: PreparePayData,

    // unserializable fields
    swap_script: SwapScript,
    chain_client: Arc<ChainClient>,
    api: Arc<BoltzApiClientV2>,
    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    polling: bool,
    timeout_advance: Duration,
}

impl BoltzSession {
    pub async fn prepare_pay(
        &self,
        lightning_payment: &LightningPayment,
        refund_address: &elements::Address,
        webhook: Option<Webhook<SubSwapStates>>,
    ) -> Result<PreparePayResponse, Error> {
        let chain = self.chain();

        let (bolt11_invoice_str, bolt11_invoice) = match lightning_payment {
            LightningPayment::Bolt11(invoice) => (invoice.to_string(), invoice),
            LightningPayment::Bolt12(_) => {
                return Err(Error::Bolt12Unsupported);
            }
        };
        let webhook_str = format!("{:?}", webhook);

        let (key_index, our_keys) = self.derive_next_keypair()?;
        let refund_public_key = PublicKey {
            inner: our_keys.public_key(),
            compressed: true,
        };

        if let Some((address, amount)) =
            check_for_mrh(&self.api, &bolt11_invoice_str, chain).await?
        {
            let asset_id = self.network().policy_asset().to_string();
            let mrh_uri = format!(
                "liquidnetwork:{address}?amount={}&assetid={}",
                amount.to_string_in(Denomination::Bitcoin),
                asset_id
            );
            return Err(Error::MagicRoutingHint {
                address: address.to_string(),
                amount: amount.to_sat(),
                uri: mrh_uri,
            });
        }

        let create_swap_req = CreateSubmarineRequest {
            from: chain.to_string(),
            to: "BTC".to_string(),
            invoice: bolt11_invoice_str.clone(),
            refund_public_key,
            pair_hash: None,
            referral_id: self.referral_id.clone(),
            webhook,
        };

        let create_swap_response = self.api.post_swap_req(&create_swap_req).await?;
        log::info!(
            "accept zero conf: {}",
            create_swap_response.accept_zero_conf
        );
        let bolt11_amount = bolt11_invoice
            .amount_milli_satoshis()
            .ok_or(Error::InvoiceWithoutAmount(bolt11_invoice_str.clone()))?
            / 1000;
        let fee = create_swap_response
            .expected_amount
            .checked_sub(bolt11_amount)
            .ok_or(Error::ExpectedAmountLowerThanInvoice(
                create_swap_response.expected_amount,
                bolt11_invoice_str.clone(),
            ))?;

        log::info!("Got Swap Response from Boltz server {create_swap_response:?}");

        create_swap_response.validate(&bolt11_invoice_str, &refund_public_key, chain)?;
        log::info!("VALIDATED RESPONSE!");

        let swap_script =
            SwapScript::submarine_from_swap_resp(chain, &create_swap_response, refund_public_key)?;
        let swap_id = create_swap_response.id.clone();
        log::info!(
            "Created Swap Script id:{swap_id} swap_script:{swap_script:?} webhook:{webhook_str}"
        );

        let mut rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;

        let _update = next_status(&mut rx, self.timeout, &swap_id, false).await?;

        log::info!(
            "Send {} sats to {} address {} or use uri {}",
            create_swap_response.expected_amount,
            chain,
            create_swap_response.address,
            create_swap_response.bip21
        );
        Ok(PreparePayResponse {
            polling: self.polling,
            timeout_advance: self.timeout_advance,
            data: PreparePayData {
                last_state: SwapState::InvoiceSet,
                swap_type: SwapType::Submarine,
                fee: Some(fee),
                bolt11_invoice: Some((**bolt11_invoice).clone()),
                our_keys,
                refund_address: refund_address.to_string(),
                create_swap_response: create_swap_response.clone(),
                key_index,
                mnemonic_identifier: mnemonic_identifier(&self.mnemonic)?,
            },
            swap_script: swap_script.clone(),
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }

    pub async fn restore_prepare_pay(
        &self,
        data: PreparePayDataSerializable,
    ) -> Result<PreparePayResponse, Error> {
        let data = to_prepare_pay_data(data, &self.mnemonic)?;
        let p = data.our_keys.public_key();
        let swap_script = SwapScript::submarine_from_swap_resp(
            self.chain(),
            &data.create_swap_response,
            PublicKey {
                inner: p,
                compressed: true,
            },
        )?;
        let swap_id = data.create_swap_response.id.clone();
        let rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;

        Ok(PreparePayResponse {
            polling: self.polling,
            timeout_advance: self.timeout_advance,
            data,
            swap_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }

    /// From the swaps returned by the boltz api via [`BoltzSession::swap_restore`]:
    ///
    /// - filter the submarine swaps that can be restored
    /// - Add the private information from the session needed to restore the swap
    ///
    /// The refund address doesn't need to be the same used when creating the swap.
    pub async fn restorable_submarine_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        refund_address: &elements::Address,
    ) -> Result<Vec<PreparePayData>, Error> {
        swaps
            .iter()
            .filter(|e| matches!(e.swap_type, SwapRestoreType::Submarine))
            .filter(|e| e.status != "swap.expired" && e.status != "transaction.claimed")
            .map(|e| {
                convert_swap_restore_response_to_prepare_pay_data(
                    e,
                    &self.mnemonic,
                    &refund_address.to_string(),
                )
            })
            .collect()
    }
}

pub(crate) fn convert_swap_restore_response_to_prepare_pay_data(
    e: &boltz_client::boltz::SwapRestoreResponse,
    mnemonic: &Mnemonic,
    refund_address: &str,
) -> Result<PreparePayData, Error> {
    // Only handle submarine swaps for now
    match e.swap_type {
        SwapRestoreType::Submarine => {}
        _ => {
            return Err(Error::SwapRestoration(format!(
                "Only submarine swaps are supported for restoration, got: {:?}",
                e.swap_type
            )))
        }
    }

    // Extract refund details (required for submarine swaps)
    let refund_details: &RefundDetails = e.refund_details.as_ref().ok_or_else(|| {
        Error::SwapRestoration(format!("Submarine swap {} is missing refund_details", e.id))
    })?;

    // Derive the keypair from the mnemonic at the key_index
    let our_keys = crate::derive_keypair(refund_details.key_index, mnemonic)?;

    // Parse the server public key
    let claim_public_key_bitcoin = BitcoinPublicKey::from_str(&refund_details.server_public_key)
        .map_err(|e| Error::SwapRestoration(format!("Failed to parse server public key: {}", e)))?;
    let claim_public_key = PublicKey {
        inner: claim_public_key_bitcoin.inner,
        compressed: claim_public_key_bitcoin.compressed,
    };

    // Reconstruct CreateSubmarineResponse from RefundDetails
    let create_swap_response = CreateSubmarineResponse {
        accept_zero_conf: false, // Default for restored swaps
        address: refund_details.lockup_address.clone(),
        bip21: String::new(), // Not available in restore response
        claim_public_key,
        expected_amount: 0, // Not available in restore response
        id: e.id.clone(),
        referral_id: None, // This is important only at creation time
        swap_tree: refund_details.tree.clone(),
        timeout_block_height: refund_details.timeout_block_height as u64,
        blinding_key: refund_details.blinding_key.clone(),
    };

    // Parse the status to SwapState
    let last_state = e.status.parse::<SwapState>().map_err(|err| {
        Error::SwapRestoration(format!(
            "Failed to parse status '{}' as SwapState: {}",
            e.status, err
        ))
    })?;

    Ok(PreparePayData {
        last_state,
        swap_type: SwapType::Submarine,
        fee: None,            // Fee information not available in restore response
        bolt11_invoice: None, // Invoice information not available in restore response
        our_keys,
        refund_address: refund_address.to_string(),
        create_swap_response,
        key_index: refund_details.key_index,
        mnemonic_identifier: mnemonic_identifier(mnemonic)?,
    })
}

impl PreparePayResponse {
    async fn next_status(&mut self) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id();
        next_status(&mut self.rx, self.timeout_advance, &swap_id, self.polling).await
    }

    async fn handle_cooperative_claim(
        &self,
        update: SwapStatus,
    ) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        log::info!("submarine_cooperative_claim");
        if let Some(bolt_11_invoice) = &self.data.bolt11_invoice {
            let response = self
                .swap_script
                .submarine_cooperative_claim(
                    &self.swap_id(),
                    &self.data.our_keys,
                    &bolt_11_invoice.to_string(),
                    &self.api,
                )
                .await;

            match response {
                Ok(val) => {
                    log::info!("succesfully sent submarine cooperative claim, response: {val:?}");
                    Ok(ControlFlow::Continue(update))
                }
                Err(e) => {
                    if e.to_string()
                        .contains("swap not eligible for a cooperative claim")
                    {
                        log::info!("swap not eligible for a cooperative claim, too small, boltz decision, we did our best");
                        Ok(ControlFlow::Break(true))
                    } else {
                        Err(e.into())
                    }
                }
            }
        } else {
            // we can't cooperative claim if we don't have the invoice,
            // but the payement has been succesfull, boltz will sweep anyway it will just be more expensive
            Ok(ControlFlow::Break(true))
        }
    }

    pub async fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let update = self.next_status().await?;
        let update_status = update.swap_state()?;

        let flow = match update_status {
            SwapState::InvoiceSet => Ok(ControlFlow::Continue(update)),
            SwapState::TransactionMempool => {
                log::info!("transaction.mempool Boltz broadcasted funding tx");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionLockupFailed | SwapState::InvoiceFailedToPay => {
                log::warn!("transaction.lockupFailed Boltz failed to lockup funding tx");
                sleep(WAIT_TIME).await;
                let tx = self
                    .swap_script
                    .construct_refund(SwapTransactionParams {
                        keys: self.data.our_keys,
                        output_address: self.data.refund_address.to_string(),
                        fee: Fee::Relative(1.0), // TODO: improve
                        swap_id: self.swap_id(),
                        chain_client: &self.chain_client,
                        boltz_client: &self.api,
                        options: None,
                    })
                    .await?;

                let txid = broadcast_tx_with_retry(&self.chain_client, &tx).await?;
                log::info!("Cooperative Refund Successfully broadcasted: {txid}");

                Ok(ControlFlow::Break(true))
            }
            SwapState::TransactionClaimPending => self.handle_cooperative_claim(update).await,
            SwapState::TransactionConfirmed => Ok(ControlFlow::Continue(update)),
            SwapState::InvoicePending => Ok(ControlFlow::Continue(update)),
            SwapState::InvoicePaid => Ok(ControlFlow::Continue(update)),
            SwapState::TransactionClaimed => {
                log::info!("transaction.claimed Boltz claimed funding tx");
                Ok(ControlFlow::Break(true))
            }
            SwapState::SwapExpired => {
                log::warn!("swap.expired Boltz swap expired");

                // TODO: Non cooperative refund if needed

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
        let s: PreparePayDataSerializable = self.data.clone().into();
        Ok(serde_json::to_string(&s)?)
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

    pub fn swap_id(&self) -> String {
        self.data.create_swap_response.id.clone()
    }

    pub fn address(&self) -> String {
        self.data.create_swap_response.address.clone()
    }
    pub fn amount(&self) -> u64 {
        self.data.create_swap_response.expected_amount
    }
}
