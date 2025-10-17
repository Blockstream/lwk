use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use bip39::Mnemonic;
use boltz_client::boltz::{
    BoltzApiClientV2, CreateSubmarineRequest, CreateSubmarineResponse, RefundDetails,
    SubSwapStates, SwapRestoreType, SwapStatus, Webhook,
};
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::{ChainClient, SwapScript, SwapTransactionParams};
use boltz_client::util::sleep;
use boltz_client::{Bolt11Invoice, PublicKey, Secp256k1};
use lwk_wollet::bitcoin::{Denomination, PublicKey as BitcoinPublicKey};
use lwk_wollet::elements;
use lwk_wollet::secp256k1::All;

use crate::error::Error;
use crate::prepare_pay_data::PreparePayData;
use crate::swap_state::SwapStateTrait;
use crate::{next_status, LightningSession, SwapState, SwapType, WAIT_TIME};

pub struct PreparePayResponse {
    pub data: PreparePayData,

    // unserializable fields
    swap_script: SwapScript,
    chain_client: Arc<ChainClient>,
    api: Arc<BoltzApiClientV2>,
    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
}

impl LightningSession {
    pub async fn prepare_pay(
        &self,
        bolt11_invoice: &Bolt11Invoice,
        refund_address: &elements::Address,
        webhook: Option<Webhook<SubSwapStates>>,
    ) -> Result<PreparePayResponse, Error> {
        let chain = self.chain();
        let bolt11_invoice_str = bolt11_invoice.to_string();
        let webhook_str = format!("{:?}", webhook);

        let our_keys = self.derive_next_keypair()?;
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
            referral_id: None,
            webhook,
        };

        let create_swap_response = self.api.post_swap_req(&create_swap_req).await?;

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

        let _update = next_status(
            &mut rx,
            self.timeout,
            &[SwapState::InvoiceSet],
            &swap_id,
            SwapState::Initialized,
        )
        .await?;

        log::info!(
            "Send {} sats to {} address {} or use uri {}",
            create_swap_response.expected_amount,
            chain,
            create_swap_response.address,
            create_swap_response.bip21
        );
        Ok(PreparePayResponse {
            data: PreparePayData {
                last_state: SwapState::InvoiceSet,
                swap_type: SwapType::Submarine,
                fee: Some(fee),
                bolt11_invoice: Some(bolt11_invoice.clone()),
                our_keys,
                refund_address: refund_address.to_string(),
                create_swap_response: create_swap_response.clone(),
            },
            swap_script: swap_script.clone(),
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }

    pub async fn restore_prepare_pay(
        &self,
        data: PreparePayData,
    ) -> Result<PreparePayResponse, Error> {
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
            data,
            swap_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }
}

pub(crate) fn convert_swap_restore_response_to_prepare_pay_data(
    e: &boltz_client::boltz::SwapRestoreResponse,
    mnemonic: &Mnemonic,
    secp: &Secp256k1<All>,
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
    let our_keys = crate::derive_keypair(refund_details.key_index, mnemonic, secp)?;

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
        referral_id: None,
        swap_tree: refund_details.tree.clone(),
        timeout_block_height: refund_details.timeout_block_height as u64,
        blinding_key: Some(refund_details.blinding_key.clone()),
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
    })
}

impl PreparePayResponse {
    async fn next_status(&mut self, expected_states: &[SwapState]) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id();
        next_status(
            &mut self.rx,
            Duration::from_secs(180),
            expected_states,
            &swap_id,
            self.data.last_state,
        )
        .await
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
        match self.data.last_state {
            SwapState::InvoiceSet => {
                let update = self
                    .next_status(&[
                        SwapState::TransactionMempool,
                        SwapState::TransactionLockupFailed,
                        SwapState::InvoiceSet, // we can receive the invoice set again when we restore
                        SwapState::TransactionClaimPending, // this can happen if we skip all the updates and receive the claim pending update directly
                    ])
                    .await?;
                let update_status = update.swap_state()?;
                if update_status == SwapState::InvoiceSet {
                    // TODO: this can cause an infinite loop if boltz goes crazy,
                    // should we maintain a flag and error out if hitten more than once?
                    return Ok(ControlFlow::Continue(update));
                }

                if update_status == SwapState::TransactionMempool {
                    log::info!("transaction.mempool Boltz broadcasted funding tx");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Continue(update))
                } else if update_status == SwapState::TransactionLockupFailed {
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

                    let txid = self.chain_client.broadcast_tx(&tx).await?;
                    log::info!("Cooperative Refund Successfully broadcasted: {txid}");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Break(true))
                } else if update_status == SwapState::TransactionClaimPending {
                    self.handle_cooperative_claim(update).await
                } else {
                    todo!()
                }
            }
            SwapState::TransactionMempool => {
                let update = self
                    .next_status(&[SwapState::TransactionConfirmed, SwapState::InvoicePending])
                    .await?;
                self.data.last_state = update.swap_state()?;
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionConfirmed => {
                let update = self.next_status(&[SwapState::InvoicePending]).await?;
                self.data.last_state = update.swap_state()?;
                Ok(ControlFlow::Continue(update))
            }
            SwapState::InvoicePending => {
                let update = self.next_status(&[SwapState::InvoicePaid]).await?;
                self.data.last_state = update.swap_state()?;
                Ok(ControlFlow::Continue(update))
            }
            SwapState::InvoicePaid => {
                let update = self
                    .next_status(&[SwapState::TransactionClaimPending])
                    .await?;
                self.data.last_state = update.swap_state()?;

                self.handle_cooperative_claim(update).await
            }
            SwapState::TransactionClaimPending => {
                let update = self.next_status(&[SwapState::TransactionClaimed]).await?;
                self.data.last_state = update.swap_state()?;
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionClaimed => {
                log::info!("transaction.claimed Boltz claimed funding tx");
                Ok(ControlFlow::Break(true))
            }
            ref e => Err(Error::UnexpectedUpdate {
                swap_id: self.swap_id(),
                status: e.to_string(),
                last_state: self.data.last_state,
                expected_states: vec![],
            }),
        }
    }

    pub fn serialize(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(&self.data)?)
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
