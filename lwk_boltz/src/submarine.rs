use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::{
    BoltzApiClientV2, CreateSubmarineRequest, CreateSubmarineResponse, SwapStatus,
};
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::{ChainClient, SwapScript, SwapTransactionParams};
use boltz_client::util::sleep;
use boltz_client::{Bolt11Invoice, Keypair, PublicKey, Secp256k1, ToHex};
use lwk_wollet::bitcoin::Denomination;
use lwk_wollet::elements;
use lwk_wollet::secp256k1::rand::thread_rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;
use crate::{next_status, LightningSession, SwapState, WAIT_TIME};

pub struct PreparePayResponse {
    pub data: PreparePayData,

    // unserializable fields
    swap_script: SwapScript,
    chain_client: Arc<ChainClient>,
    api: Arc<BoltzApiClientV2>,
    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
}

#[derive(Clone, Debug)]
pub struct PreparePayData {
    pub last_state: SwapState,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    pub fee: u64,
    pub bolt11_invoice: Bolt11Invoice,
    pub create_swap_response: CreateSubmarineResponse,
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
        state.serialize_field("fee", &self.fee)?;
        state.serialize_field("bolt11_invoice", &self.bolt11_invoice.to_string())?;
        state.serialize_field("create_swap_response", &self.create_swap_response)?;
        // Serialize the secret key hex string for keypair recreation
        state.serialize_field("secret_key", &self.our_keys.secret_bytes().to_hex())?;
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
            fee: u64,
            bolt11_invoice: String,
            create_swap_response: CreateSubmarineResponse,
            secret_key: String, // Secret key hex string
            refund_address: String,
        }

        let helper = PreparePayDataHelper::deserialize(deserializer)?;

        // Parse bolt11_invoice from string
        let bolt11_invoice = match Bolt11Invoice::from_str(&helper.bolt11_invoice) {
            Ok(invoice) => invoice,
            Err(_) => return Err(serde::de::Error::custom("Failed to parse bolt11 invoice")),
        };

        // Recreate Keypair from secret key bytes using from_seckey_slice
        let secp = Secp256k1::new();
        let our_keys = match Keypair::from_seckey_str(&secp, &helper.secret_key) {
            Ok(keypair) => keypair,
            Err(_) => {
                return Err(serde::de::Error::custom(
                    "Failed to recreate keypair from secret key",
                ))
            }
        };

        Ok(PreparePayData {
            last_state: helper.last_state,
            fee: helper.fee,
            bolt11_invoice,
            create_swap_response: helper.create_swap_response,
            our_keys,
            refund_address: helper.refund_address,
        })
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
                fee,
                bolt11_invoice: bolt11_invoice.clone(),
                our_keys: our_keys.clone(),
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
        let rx = self.ws.updates();
        self.ws
            .subscribe_swap(&data.create_swap_response.id)
            .await?;
        Ok(PreparePayResponse {
            data,
            swap_script,
            rx,
            api: self.api.clone(),
            chain_client: self.chain_client.clone(),
        })
    }
}

impl PreparePayResponse {
    async fn next_status(&mut self, expected_states: &[SwapState]) -> Result<SwapStatus, Error> {
        let swap_id = self.swap_id();
        next_status(
            &mut self.rx,
            Duration::from_secs(180),
            expected_states,
            &swap_id,
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
                    .swap_script
                    .submarine_cooperative_claim(
                        &self.swap_id(),
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
                swap_id: self.swap_id(),
                status: e.to_string(),
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

    fn swap_id(&self) -> String {
        self.data.create_swap_response.id.clone()
    }
}

impl PreparePayData {
    pub fn deserialize(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_pay_data_serialization_roundtrip() {
        // Load the JSON data from the test file
        let json_data = include_str!("../tests/data/prepare_pay_response.json");

        // Deserialize the JSON into PreparePayData
        let deserialized1: PreparePayData = serde_json::from_str(json_data)
            .expect("Failed to deserialize PreparePayData from JSON");

        // Serialize it back to JSON
        let serialized = serde_json::to_string(&deserialized1)
            .expect("Failed to serialize PreparePayData to JSON");

        // Deserialize again
        let deserialized2: PreparePayData = serde_json::from_str(&serialized)
            .expect("Failed to deserialize PreparePayData from serialized JSON");

        // Compare the two deserialized versions for equality
        assert_eq!(deserialized1.last_state, deserialized2.last_state);
        assert_eq!(deserialized1.fee, deserialized2.fee);
        assert_eq!(
            deserialized1.bolt11_invoice.to_string(),
            deserialized2.bolt11_invoice.to_string()
        );
        assert_eq!(
            deserialized1.create_swap_response.id,
            deserialized2.create_swap_response.id
        );
        assert_eq!(
            deserialized1.create_swap_response.expected_amount,
            deserialized2.create_swap_response.expected_amount
        );
        assert_eq!(
            deserialized1.create_swap_response.address,
            deserialized2.create_swap_response.address
        );
        assert_eq!(
            deserialized1.our_keys.secret_bytes(),
            deserialized2.our_keys.secret_bytes()
        );
        assert_eq!(deserialized1.refund_address, deserialized2.refund_address);

        // Also test that the full structs are equal (this will test all fields)
        // Note: We can't directly compare PreparePayData due to Keypair not implementing Eq
        // But we can compare all the individual fields as above
    }
}
