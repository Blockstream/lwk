use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::{BoltzApiClientV2, CreateSubmarineRequest};
use boltz_client::fees::Fee;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::{ChainClient, SwapScript, SwapTransactionParams};
use boltz_client::util::sleep;
use boltz_client::{Bolt11Invoice, Keypair, PublicKey, Secp256k1};
use lwk_wollet::bitcoin::Denomination;
use lwk_wollet::elements;
use lwk_wollet::secp256k1::rand::thread_rng;

use crate::error::Error;
use crate::{LightningSession, WAIT_TIME};

pub struct PreparePayResponse {
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

    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    bolt11_invoice: Bolt11Invoice,
    swap_script: SwapScript,
    api: Arc<BoltzApiClientV2>,
    our_keys: Keypair,
    chain_client: Arc<ChainClient>,
    refund_address: String,
}

impl LightningSession {
    pub async fn prepare_pay(
        &self,
        bolt11_invoice: &Bolt11Invoice,
        refund_address: &elements::Address,
    ) -> Result<PreparePayResponse, Error> {
        let chain = self.chain();

        let secp = Secp256k1::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());
        let refund_public_key = PublicKey {
            inner: our_keys.public_key(),
            compressed: true,
        };

        if let Some((address, amount)) =
            check_for_mrh(&self.api, &bolt11_invoice.to_string(), chain).await?
        {
            let mrh_uri = format!(
                "liquidnetwork:{address}?amount={:.8}",
                amount.to_string_in(Denomination::Bitcoin)
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
            invoice: bolt11_invoice.to_string(),
            refund_public_key,
            pair_hash: None,
            referral_id: None,
            webhook: None,
        };

        let create_swap_response = self.api.post_swap_req(&create_swap_req).await?;

        let bolt11_amount = bolt11_invoice
            .amount_milli_satoshis()
            .ok_or(Error::InvoiceWithoutAmount(bolt11_invoice.to_string()))?
            / 1000;
        let fee = create_swap_response
            .expected_amount
            .checked_sub(bolt11_amount)
            .ok_or(Error::ExpectedAmountLowerThanInvoice(
                create_swap_response.expected_amount,
                bolt11_invoice.to_string(),
            ))?;

        log::info!("Got Swap Response from Boltz server {create_swap_response:?}");

        create_swap_response.validate(&bolt11_invoice.to_string(), &refund_public_key, chain)?;
        log::info!("VALIDATED RESPONSE!");

        let swap_script =
            SwapScript::submarine_from_swap_resp(chain, &create_swap_response, refund_public_key)?;
        let swap_id = create_swap_response.id.clone();
        log::info!("Created Swap Script id:{swap_id} swap_script:{swap_script:?}");

        let mut rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await?;
        let update = tokio::select! {
            update = rx.recv() => update?,
            _ = tokio::time::sleep(self.timeout) => {
                return Err(Error::Timeout(swap_id.clone()));
            }
        };
        match update.status.as_str() {
            "invoice.set" => {
                log::info!(
                    "Send {} sats to {} address {}",
                    create_swap_response.expected_amount,
                    chain,
                    create_swap_response.address
                );
                Ok(PreparePayResponse {
                    swap_id,
                    uri: create_swap_response.bip21,
                    address: create_swap_response.address,
                    amount: create_swap_response.expected_amount,
                    fee,
                    rx,
                    swap_script,
                    api: self.api.clone(),
                    our_keys,
                    chain_client: self.chain_client.clone(),
                    refund_address: refund_address.to_string(),
                    bolt11_invoice: bolt11_invoice.clone(),
                })
            }
            _ => Err(Error::UnexpectedUpdate {
                swap_id: update.id,
                status: update.status,
            }),
        }
    }
}

impl PreparePayResponse {
    pub async fn complete_pay(mut self) -> Result<bool, Error> {
        loop {
            let update = tokio::select! {
                update = self.rx.recv() => update?,
                _ = tokio::time::sleep(Duration::from_secs(180)) => {
                    // We use a conservartively long 3 minute timeout because the swap can take a
                    // while to complete and also block confirmation may
                    return Err(Error::Timeout(self.swap_id.clone()));
                }
            };
            log::info!("Got Update from server: {}", update.status);
            log::debug!("Update: {update:?}");
            match update.status.as_str() {
                "transaction.mempool" => {}
                "transaction.confirmed" => {}
                "invoice.pending" => {}
                "invoice.paid" => {}
                "transaction.claim.pending" => {
                    let response = self
                        .swap_script
                        .submarine_cooperative_claim(
                            &self.swap_id,
                            &self.our_keys,
                            &self.bolt11_invoice.to_string(),
                            &self.api,
                        )
                        .await?;
                    log::debug!("Received claim tx details : {response:?}");
                }

                "transaction.claimed" => {
                    break Ok(true);
                }

                // This means the funding transaction was rejected by Boltz for whatever reason, and we need to get
                // the funds back via refund.
                "transaction.lockupFailed" | "invoice.failedToPay" => {
                    sleep(WAIT_TIME).await;
                    let tx = self
                        .swap_script
                        .construct_refund(SwapTransactionParams {
                            keys: self.our_keys,
                            output_address: self.refund_address.to_string(),
                            fee: Fee::Relative(1.0), // TODO: improve
                            swap_id: self.swap_id.clone(),
                            chain_client: &self.chain_client,
                            boltz_client: &self.api,
                            options: None,
                        })
                        .await
                        .unwrap();

                    let txid = self.chain_client.broadcast_tx(&tx).await.unwrap();
                    log::info!("Cooperative Refund Successfully broadcasted: {txid}");
                    break Ok(false);

                    // Non cooperative refund requires expired swap
                    /*log::info!("Cooperative refund failed. {:?}", e);
                    log::info!("Attempting Non-cooperative refund.");

                    let tx = swap_tx
                        .sign_refund(&our_keys, Fee::Absolute(1000), None)
                        .await
                        .unwrap();
                    let txid = swap_tx
                        .broadcast(&tx, bitcoin_client)
                        .await
                        .unwrap();
                    log::info!("Non-cooperative Refund Successfully broadcasted: {}", txid);*/
                }
                _ => {
                    Err(Error::UnexpectedUpdate {
                        swap_id: self.swap_id.clone(),
                        status: update.status,
                    })?;
                }
            };
        }
    }
}
