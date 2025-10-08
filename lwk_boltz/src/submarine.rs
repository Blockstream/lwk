use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::{BoltzApiClientV2, CreateSubmarineRequest, SwapStatus};
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::{ChainClient, SwapScript, SwapTransactionParams};
use boltz_client::util::sleep;
use boltz_client::{Bolt11Invoice, Keypair, PublicKey, Secp256k1};
use lwk_wollet::bitcoin::Denomination;
use lwk_wollet::elements;
use lwk_wollet::secp256k1::rand::thread_rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;
use crate::{next_status, LightningSession, SwapState, WAIT_TIME};

pub struct PreparePayResponse {
    pub data: PreparePayData,

    // unserializable fields
    chain_client: Arc<ChainClient>,
    api: Arc<BoltzApiClientV2>,
    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
}

#[derive(Clone, Debug)]
pub struct PreparePayData {
    pub last_state: SwapState,
    pub swap_id: String,

    /// A liquidnetwork uri with the address to pay and the amount.
    /// Note the amount is greater that what is specified in the bolt11 invoice because of fees
    pub uri: String,

    /// The address to pay to.
    /// It is the same contained in the uri but provided for convenience.
    pub address: String,

    /// The amount to pay.
    /// It is the same contained in the uri but provided for convenience.
    pub amount: u64,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    pub fee: u64,
    pub bolt11_invoice: Bolt11Invoice,
    pub swap_script: SwapScript,
    pub our_keys: Keypair,
    pub refund_address: String,
}

impl Serialize for PreparePayData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("PreparePayData", 10)?;
        state.serialize_field("last_state", &self.last_state)?;
        state.serialize_field("swap_id", &self.swap_id)?;
        state.serialize_field("uri", &self.uri)?;
        state.serialize_field("address", &self.address)?;
        state.serialize_field("amount", &self.amount)?;
        state.serialize_field("fee", &self.fee)?;
        state.serialize_field("bolt11_invoice", &self.bolt11_invoice.to_string())?;
        // TODO: Implement proper serialization for SwapScript
        state.serialize_field(
            "swap_script",
            &"TODO: SwapScript serialization not implemented",
        )?;
        // TODO: Implement proper serialization for Keypair (this contains private keys - be careful!)
        state.serialize_field(
            "our_keys",
            &"TODO: Keypair serialization not implemented - contains private keys",
        )?;
        state.serialize_field("refund_address", &self.refund_address)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PreparePayData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PreparePayDataHelper {
            last_state: SwapState,
            swap_id: String,
            uri: String,
            address: String,
            amount: u64,
            fee: u64,
            bolt11_invoice: String,
            swap_script: String,
            our_keys: String,
            refund_address: String,
        }

        let helper = PreparePayDataHelper::deserialize(deserializer)?;

        // Parse bolt11_invoice from string
        let _bolt11_invoice = match Bolt11Invoice::from_str(&helper.bolt11_invoice) {
            Ok(invoice) => invoice,
            Err(_) => return Err(serde::de::Error::custom("Failed to parse bolt11 invoice")),
        };

        // TODO: Implement deserialization for SwapScript
        todo!("SwapScript deserialization not implemented");

        // TODO: Implement deserialization for Keypair - this is particularly challenging since it contains private keys
        todo!(
            "Keypair deserialization not implemented - contains private keys, need secure handling"
        );
    }
}

impl LightningSession {
    pub async fn prepare_pay(
        &self,
        bolt11_invoice: &Bolt11Invoice,
        refund_address: &elements::Address,
    ) -> Result<PreparePayResponse, Error> {
        let chain = self.chain();
        let bolt11_invoice_str = bolt11_invoice.to_string();

        let secp = Secp256k1::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());
        let refund_public_key = PublicKey {
            inner: our_keys.public_key(),
            compressed: true,
        };

        if let Some((address, amount)) =
            check_for_mrh(&self.api, &bolt11_invoice_str, chain).await?
        {
            let asset_id = self.network().policy_asset().to_string();
            let mrh_uri = format!(
                "liquidnetwork:{address}?amount={:.8}&assetid={}",
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
            webhook: None,
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
        log::info!("Created Swap Script id:{swap_id} swap_script:{swap_script:?}");

        let mut rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;

        let _update =
            next_status(&mut rx, self.timeout, &[SwapState::InvoiceSet], &swap_id).await?;

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
                swap_id,
                uri: create_swap_response.bip21,
                address: create_swap_response.address,
                amount: create_swap_response.expected_amount,
                fee,
                bolt11_invoice: bolt11_invoice.clone(),
                swap_script: swap_script.clone(),
                our_keys: our_keys.clone(),
                refund_address: refund_address.to_string(),
            },
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }
}

impl PreparePayResponse {
    async fn next_status(&mut self, expected_states: &[SwapState]) -> Result<SwapStatus, Error> {
        next_status(
            &mut self.rx,
            Duration::from_secs(180),
            expected_states,
            &self.data.swap_id,
        )
        .await
    }

    async fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        match self.data.last_state {
            SwapState::InvoiceSet => {
                let update = self
                    .next_status(&[
                        SwapState::TransactionMempool,
                        SwapState::TransactionLockupFailed,
                    ])
                    .await?;
                let update_status = update.status.parse::<SwapState>().expect("TODO");

                if update_status == SwapState::TransactionMempool {
                    log::info!("transaction.mempool Boltz broadcasted funding tx");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Continue(update))
                } else if update_status == SwapState::TransactionLockupFailed {
                    log::warn!("transaction.lockupFailed Boltz failed to lockup funding tx");
                    sleep(WAIT_TIME).await;
                    let tx = self
                        .data
                        .swap_script
                        .construct_refund(SwapTransactionParams {
                            keys: self.data.our_keys,
                            output_address: self.data.refund_address.to_string(),
                            fee: Fee::Relative(1.0), // TODO: improve
                            swap_id: self.data.swap_id.clone(),
                            chain_client: &self.chain_client,
                            boltz_client: &self.api,
                            options: None,
                        })
                        .await
                        .unwrap();

                    let txid = self.chain_client.broadcast_tx(&tx).await.unwrap();
                    log::info!("Cooperative Refund Successfully broadcasted: {txid}");
                    self.data.last_state = update_status;
                    Ok(ControlFlow::Break(true))
                } else {
                    todo!()
                }
            }
            SwapState::TransactionMempool => {
                let update = self.next_status(&[SwapState::TransactionConfirmed]).await?;
                self.data.last_state = update.status.parse::<SwapState>().expect("TODO");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionConfirmed => {
                let update = self.next_status(&[SwapState::InvoicePending]).await?;
                self.data.last_state = update.status.parse::<SwapState>().expect("TODO");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::InvoicePending => {
                let update = self.next_status(&[SwapState::InvoicePaid]).await?;
                self.data.last_state = update.status.parse::<SwapState>().expect("TODO");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::InvoicePaid => {
                let update = self
                    .next_status(&[SwapState::TransactionClaimPending])
                    .await?;
                self.data.last_state = update.status.parse::<SwapState>().expect("TODO");
                let response = self
                    .data
                    .swap_script
                    .submarine_cooperative_claim(
                        &self.data.swap_id,
                        &self.data.our_keys,
                        &self.data.bolt11_invoice.to_string(),
                        &self.api,
                    )
                    .await?;
                log::debug!("Received claim tx details : {response:?}");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionClaimPending => {
                let update = self.next_status(&[SwapState::TransactionClaimed]).await?;
                self.data.last_state = update.status.parse::<SwapState>().expect("TODO");
                Ok(ControlFlow::Continue(update))
            }
            SwapState::TransactionClaimed => {
                log::info!("transaction.claimed Boltz claimed funding tx");
                Ok(ControlFlow::Break(true))
            }
            ref e => Err(Error::UnexpectedUpdate {
                swap_id: self.data.swap_id.clone(),
                status: e.to_string(),
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
