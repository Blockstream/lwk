use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::CreateReverseRequest;
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
use boltz_client::{Bolt11Invoice, Keypair, PublicKey};
use lwk_wollet::elements;
use lwk_wollet::secp256k1::rand::thread_rng;

use crate::error::Error;
use crate::{next_status, LightningSession, SwapState};

pub struct InvoiceResponse {
    pub swap_id: String,
    /// The invoice to show to the payer, the invoice amount will be exactly like the amount parameter,
    /// However, the receiver will receive `amount - fee`
    pub bolt11_invoice: Bolt11Invoice,

    /// The fee of the swap provider
    pub fee: u64,

    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    swap_script: SwapScript,
    api: Arc<BoltzApiClientV2>,
    our_keys: Keypair,
    preimage: Preimage,
    claim_address: elements::Address,
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
            swap_id,
            bolt11_invoice: invoice,
            fee,
            rx,
            swap_script,
            api: self.api.clone(),
            our_keys,
            preimage,
            claim_address: claim_address.clone(),
            chain_client: self.chain_client.clone(),
        })
    }
}

impl InvoiceResponse {
    async fn next_status(&mut self, expected_states: &[SwapState]) -> Result<SwapStatus, Error> {
        next_status(
            &mut self.rx,
            Duration::from_secs(180),
            expected_states,
            &self.swap_id,
            SwapState::TemporaryDeleteMe, // TODO: use self.last_state once available
        )
        .await
    }
    pub async fn complete_pay(mut self) -> Result<bool, Error> {
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
            return Ok(true);
        } else {
            log::info!("transaction.mempool/confirmed Boltz broadcasted funding tx");
            let tx = self
                .swap_script
                .construct_claim(
                    &self.preimage,
                    SwapTransactionParams {
                        keys: self.our_keys,
                        output_address: self.claim_address.to_string(),
                        fee: Fee::Relative(1.0),
                        swap_id: self.swap_id.clone(),
                        options: Some(TransactionOptions::default().with_cooperative(true)),
                        chain_client: &self.chain_client,
                        boltz_client: &self.api,
                    },
                )
                .await?;

            self.chain_client.broadcast_tx(&tx).await?;

            log::info!("Successfully broadcasted claim tx!");
            log::debug!("Claim Tx {tx:?}");
        }
        let _update = self.next_status(&[SwapState::InvoiceSettled]).await?; // Can we receive TransactionConfirmed before InvoiceSettled?
        log::info!("invoice.settled Reverse Swap Successful!");
        Ok(true)
    }
}
