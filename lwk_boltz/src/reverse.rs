use std::ops::ControlFlow;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::CreateReverseRequest;
use boltz_client::boltz::CreateReverseResponse;
use boltz_client::boltz::SwapStatus;
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::magic_routing::sign_address;
use boltz_client::swaps::ChainClient;
use boltz_client::swaps::SwapScript;
use boltz_client::swaps::SwapTransactionParams;
use boltz_client::swaps::TransactionOptions;
use boltz_client::util::secrets::Preimage;
use boltz_client::Secp256k1;
use boltz_client::ToHex;
use boltz_client::{Bolt11Invoice, Keypair, PublicKey};
use lwk_wollet::elements;
use lwk_wollet::secp256k1::rand::thread_rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::Error;
use crate::{next_status, LightningSession, SwapState};

#[derive(Clone, Debug)]
pub struct InvoiceData {
    pub last_state: SwapState,

    /// The fee of the swap provider
    pub fee: u64,

    create_reverse_response: CreateReverseResponse,

    our_keys: Keypair,
    preimage: Preimage,
    claim_address: elements::Address,
}

impl Serialize for InvoiceData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("InvoiceData", 8)?;
        state.serialize_field("last_state", &self.last_state)?;
        state.serialize_field("fee", &self.fee)?;
        state.serialize_field("create_reverse_response", &self.create_reverse_response)?;
        // Serialize the secret key hex string for keypair recreation
        state.serialize_field("secret_key", &self.our_keys.secret_bytes().to_hex())?;
        // Serialize the preimage using to_string
        state.serialize_field(
            "preimage",
            &self
                .preimage
                .to_string()
                .ok_or_else(|| serde::ser::Error::custom("Preimage bytes not available"))?,
        )?;
        state.serialize_field("claim_address", &self.claim_address.to_string())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for InvoiceData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct InvoiceDataHelper {
            last_state: SwapState,
            fee: u64,
            create_reverse_response: CreateReverseResponse,
            secret_key: String, // Secret key hex string
            preimage: String,   // Preimage hex string
            claim_address: String,
        }

        let helper = InvoiceDataHelper::deserialize(deserializer)?;

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

        // Parse preimage from string
        let preimage = match boltz_client::util::secrets::Preimage::from_str(&helper.preimage) {
            Ok(preimage) => preimage,
            Err(_) => return Err(serde::de::Error::custom("Failed to parse preimage")),
        };

        // Parse claim_address from string
        let claim_address = match elements::Address::from_str(&helper.claim_address) {
            Ok(address) => address,
            Err(_) => return Err(serde::de::Error::custom("Failed to parse claim address")),
        };

        Ok(InvoiceData {
            last_state: helper.last_state,
            fee: helper.fee,
            create_reverse_response: helper.create_reverse_response,
            our_keys,
            preimage,
            claim_address,
        })
    }
}

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
    ) -> Result<InvoiceResponse, Error> {
        let chain = self.chain();
        let secp = Secp256k1::new();
        let preimage = Preimage::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());
        let claim_public_key = PublicKey {
            compressed: true,
            inner: our_keys.public_key(),
        };

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
            webhook: None,
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
        log::info!("subscribing to swap: {swap_id}");
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
        log::info!("Waiting for Invoice to be paid: {}", &invoice);

        Ok(InvoiceResponse {
            data: InvoiceData {
                last_state: SwapState::SwapCreated,
                fee,
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
        Bolt11Invoice::from_str(&self.data.create_reverse_response.invoice.as_ref().expect(
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
                let update_status = update.status.parse::<SwapState>().expect("TODO");

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
                                keys: self.data.our_keys.clone(),
                                output_address: self.data.claim_address.to_string(),
                                fee: Fee::Relative(1.0),
                                swap_id: self.swap_id().to_string(),
                                options: Some(TransactionOptions::default().with_cooperative(true)),
                                chain_client: &self.chain_client,
                                boltz_client: &self.api,
                            },
                        )
                        .await?;

                    self.chain_client.broadcast_tx(&tx).await?;

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
                let update_status = update.status.parse::<SwapState>().expect("TODO");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoice_data_serialization_roundtrip() {
        // Load the JSON data from the test file
        let json_data = include_str!("../tests/data/invoice_response.json");

        // Deserialize the JSON into InvoiceData
        let deserialized1: InvoiceData =
            serde_json::from_str(json_data).expect("Failed to deserialize InvoiceData from JSON");

        // Serialize it back to JSON
        let serialized =
            serde_json::to_string(&deserialized1).expect("Failed to serialize InvoiceData to JSON");

        // Deserialize again
        let deserialized2: InvoiceData = serde_json::from_str(&serialized)
            .expect("Failed to deserialize InvoiceData from serialized JSON");

        // Compare the two deserialized versions for equality
        assert_eq!(deserialized1.last_state, deserialized2.last_state);
        assert_eq!(deserialized1.fee, deserialized2.fee);
        assert_eq!(
            deserialized1.create_reverse_response.id,
            deserialized2.create_reverse_response.id
        );
        assert_eq!(
            deserialized1.create_reverse_response.onchain_amount,
            deserialized2.create_reverse_response.onchain_amount
        );
        assert_eq!(
            deserialized1.our_keys.secret_bytes(),
            deserialized2.our_keys.secret_bytes()
        );
        assert_eq!(deserialized1.claim_address, deserialized2.claim_address);

        // Note: We can't directly compare Preimage due to its structure, but we can compare the hex strings
        assert_eq!(
            deserialized1.preimage.to_string(),
            deserialized2.preimage.to_string()
        );
    }
}
