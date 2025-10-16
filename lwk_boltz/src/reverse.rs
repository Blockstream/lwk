use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::CreateReverseRequest;
use boltz_client::boltz::RevSwapStates;
use boltz_client::boltz::SwapStatus;
use boltz_client::boltz::Webhook;
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::magic_routing::sign_address;
use boltz_client::swaps::ChainClient;
use boltz_client::swaps::SwapScript;
use boltz_client::swaps::SwapTransactionParams;
use boltz_client::swaps::TransactionOptions;
use boltz_client::util::secrets::Preimage;
use boltz_client::util::sleep;
use boltz_client::Keypair;
use boltz_client::{Bolt11Invoice, PublicKey};
use lwk_wollet::elements;
use lwk_wollet::hashes::sha256;
use lwk_wollet::hashes::Hash;

use crate::error::Error;
use crate::invoice_data::InvoiceData;
use crate::swap_state::SwapStateTrait;
use crate::SwapType;
use crate::{next_status, LightningSession, SwapState};

pub struct InvoiceResponse {
    pub data: InvoiceData,

    // unserializable fields
    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    swap_script: SwapScript,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
}

impl LightningSession {
    pub async fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &elements::Address,
        webhook: Option<Webhook<RevSwapStates>>,
    ) -> Result<InvoiceResponse, Error> {
        let chain = self.chain();
        let our_keys = self.derive_next_keypair()?;
        let preimage = preimage_from_keypair(&our_keys)?;

        let claim_public_key = PublicKey {
            compressed: true,
            inner: our_keys.public_key(),
        };
        let webhook_str = format!("{:?}", webhook);

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
            referral_id: None, // Add address signature here.
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

        let _ = check_for_mrh(&self.api, &invoice_str, chain).await?.ok_or(
            Error::InvoiceWithoutMagicRoutingHint(reverse_resp.id.clone()),
        )?;

        log::debug!("Got Reverse swap response: {reverse_resp:?}");

        let swap_script =
            SwapScript::reverse_from_swap_resp(chain, &reverse_resp, claim_public_key)?;
        let swap_id = reverse_resp.id.clone();
        log::info!("subscribing to swap: {swap_id} webhook:{webhook_str}");
        self.ws.subscribe_swap(&swap_id).await?;
        let mut rx = self.ws.updates();

        let _update = next_status(
            &mut rx,
            self.timeout,
            &[SwapState::SwapCreated],
            &swap_id,
            SwapState::Initialized,
        )
        .await?;
        log::debug!("Waiting for Invoice to be paid: {}", &invoice);

        Ok(InvoiceResponse {
            data: InvoiceData {
                last_state: SwapState::SwapCreated,
                swap_type: SwapType::Reverse,
                fee: Some(fee),
                create_reverse_response: reverse_resp.clone(),
                our_keys,
                preimage,
                claim_address: claim_address.clone(),
            },
            rx,
            swap_script,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }

    pub async fn restore_invoice(&self, data: InvoiceData) -> Result<InvoiceResponse, Error> {
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
        let mut rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;

        let state = rx.recv().await?; // skip the initial state which is resent from boltz server
        log::info!("Received initial state for swap {}: {state:?}", swap_id);

        if state.status.contains("expired") {
            return Err(Error::Expired {
                swap_id,
                status: state.status.clone(),
            });
        }

        Ok(InvoiceResponse {
            data,
            rx,
            swap_script,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }
}

fn preimage_from_keypair(our_keys: &Keypair) -> Result<Preimage, Error> {
    let hashed_bytes = sha256::Hash::hash(&our_keys.secret_bytes());
    Ok(Preimage::from_vec(hashed_bytes.as_byte_array().to_vec())?)
}

impl InvoiceResponse {
    async fn next_status(&mut self, expected_states: &[SwapState]) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id().to_string();
        next_status(
            &mut self.rx,
            Duration::from_secs(180),
            expected_states,
            &swap_id,
            self.data.last_state,
        )
        .await
    }

    pub fn swap_id(&self) -> &str {
        &self.data.create_reverse_response.id
    }

    pub fn serialize(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(&self.data)?)
    }

    pub fn bolt11_invoice(&self) -> Bolt11Invoice {
        Bolt11Invoice::from_str(self.data.create_reverse_response.invoice.as_ref().expect(
            "Invoice must be present or we would have errored on the LightningSession::invoice",
        ))
        .expect(
            "Invoice must be parsable or we would have errored on the LightningSession::invoice",
        )
    }

    pub async fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        match self.data.last_state {
            SwapState::SwapCreated => {
                let update = self
                    .next_status(&[
                        SwapState::TransactionDirect,
                        SwapState::TransactionMempool,
                        SwapState::TransactionConfirmed,
                    ])
                    .await?;
                let update_status = update.swap_state()?;

                if update_status == SwapState::TransactionDirect {
                    log::info!("transaction.direct Payer used magic routing hint");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Break(true))
                } else {
                    log::info!("transaction.mempool/confirmed Boltz broadcasted funding tx");
                    let tx = self
                        .swap_script
                        .construct_claim(
                            &self.data.preimage,
                            SwapTransactionParams {
                                keys: self.data.our_keys,
                                output_address: self.data.claim_address.to_string(),
                                fee: Fee::Relative(1.0),
                                swap_id: self.swap_id().to_string(),
                                options: Some(TransactionOptions::default().with_cooperative(true)),
                                chain_client: &self.chain_client,
                                boltz_client: &self.api,
                            },
                        )
                        .await?;

                    for _ in 0..30 {
                        match self.chain_client.broadcast_tx(&tx).await {
                            Ok(_) => break,
                            Err(_) => {
                                log::info!("Failed broadcast, retrying in 1 second");
                                sleep(Duration::from_secs(1)).await;
                            }
                        }
                    }

                    log::info!("Successfully broadcasted claim tx!");
                    log::debug!("Claim Tx {tx:?}");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Continue(update))
                }
            }
            SwapState::TransactionMempool | SwapState::TransactionConfirmed => {
                let update = self
                    .next_status(&[SwapState::InvoiceSettled, SwapState::TransactionConfirmed])
                    .await?;
                let update_status = update.swap_state()?;
                if update_status == SwapState::TransactionConfirmed {
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Continue(update))
                } else {
                    // InvoiceSettled
                    log::info!("invoice.settled Reverse Swap Successful!");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Break(true))
                }
            }
            ref e => Err(Error::UnexpectedUpdate {
                swap_id: self.swap_id().to_string(),
                status: e.to_string(),
                last_state: self.data.last_state,
                expected_states: vec![],
            }),
        }
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
