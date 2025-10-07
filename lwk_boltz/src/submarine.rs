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

use crate::error::Error;
use crate::{next_status, LightningSession, SwapState, WAIT_TIME};

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
}

impl PreparePayResponse {
    async fn next_status(&mut self, expected_states: &[SwapState]) -> Result<SwapStatus, Error> {
        next_status(
            &mut self.rx,
            Duration::from_secs(180),
            expected_states,
            &self.swap_id,
        )
        .await
    }

    pub async fn complete_pay(mut self) -> Result<bool, Error> {
        let update = self
            .next_status(&[
                SwapState::TransactionMempool,
                SwapState::TransactionLockupFailed,
            ])
            .await?;
        let update_status = update.status.parse::<SwapState>().expect("TODO");

        if update_status == SwapState::TransactionMempool {
            log::info!("transaction.mempool Boltz broadcasted funding tx");
        } else if update_status == SwapState::TransactionLockupFailed {
            log::warn!("transaction.lockupFailed Boltz failed to lockup funding tx");
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
            return Ok(false);
        }
        let _update = self.next_status(&[SwapState::TransactionConfirmed]).await?;
        let _update = self.next_status(&[SwapState::InvoicePending]).await?;
        let _update = self.next_status(&[SwapState::InvoicePaid]).await?;
        let _update = self
            .next_status(&[SwapState::TransactionClaimPending])
            .await?;

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

        let _update = self.next_status(&[SwapState::TransactionClaimed]).await?;
        log::info!("transaction.claimed Boltz claimed funding tx");
        Ok(true)
    }
}
