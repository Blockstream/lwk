#[cfg(feature = "blocking")]
mod electrum;
mod esplora;
mod waterfalls;

use std::fmt::Display;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "blocking")]
pub use electrum::ElectrumClient;
pub use esplora::EsploraClient;
pub use waterfalls::WaterfallsClient;

use async_trait::async_trait;
use boltz_client::{
    elements,
    error::Error,
    network::{LiquidChain, LiquidClient},
};
use lwk_wollet::asyncr::async_sleep;
use lwk_wollet::elements as lwk_elements;
use web_time::Instant;

// The Boltz fork uses Elements 0.27 while the rest of LWK still uses 0.25.
// Keep the version boundary here until the workspace upgrade in #576.
fn lwk_txid(txid: elements::Txid) -> Result<lwk_elements::Txid, Error> {
    txid.to_string()
        .parse()
        .map_err(|e| Error::Protocol(format!("invalid transaction id: {e}")))
}

fn boltz_tx(tx: &lwk_elements::Transaction) -> Result<elements::Transaction, Error> {
    elements::encode::deserialize(&lwk_elements::encode::serialize(tx))
        .map_err(|e| Error::Protocol(format!("invalid Elements transaction: {e}")))
}

fn lwk_tx(tx: &elements::Transaction) -> Result<lwk_elements::Transaction, Error> {
    lwk_elements::encode::deserialize(&elements::encode::serialize(tx))
        .map_err(|e| Error::Protocol(format!("invalid Elements transaction: {e}")))
}

fn boltz_utxo(
    outpoint: lwk_elements::OutPoint,
    lwk_output: &lwk_elements::TxOut,
) -> Result<(elements::OutPoint, elements::TxOut), Error> {
    let txid = outpoint
        .txid
        .to_string()
        .parse()
        .map_err(|e| Error::Protocol(format!("invalid transaction id: {e}")))?;
    let mut output: elements::TxOut =
        elements::encode::deserialize(&lwk_elements::encode::serialize(lwk_output))
            .map_err(|e| Error::Protocol(format!("invalid Elements output: {e}")))?;
    output.witness =
        elements::encode::deserialize(&lwk_elements::encode::serialize(&lwk_output.witness))
            .map_err(|e| Error::Protocol(format!("invalid Elements output witness: {e}")))?;
    Ok((
        elements::OutPoint {
            txid,
            vout: outpoint.vout,
        },
        output,
    ))
}

fn boltz_block_hash(hash: lwk_elements::BlockHash) -> Result<elements::BlockHash, Error> {
    hash.to_string()
        .parse()
        .map_err(|e| Error::Protocol(format!("invalid block hash: {e}")))
}

pub(crate) async fn wait_for_tx<Tx, F, Fut, Txid>(
    txid: Txid,
    timeout: Duration,
    interval: Duration,
    mut get_tx: F,
) -> Result<Tx, Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Tx, Error>>,
    Txid: Display + Copy,
{
    let start = Instant::now();

    loop {
        let err = match get_tx().await {
            Ok(tx) => return Ok(tx),
            Err(err) => err,
        };

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(Error::Protocol(format!(
                "timed out waiting for transaction {txid}: {err}"
            )));
        }

        async_sleep(remaining.min(interval).as_millis() as u64)
            .await
            .map_err(|err| Error::Protocol(err.to_string()))?;
    }
}

pub enum AnyClient {
    #[cfg(feature = "blocking")]
    Electrum(Arc<ElectrumClient>),
    Esplora(Arc<EsploraClient>),
    Waterfalls(Arc<WaterfallsClient>),
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl LiquidClient for AnyClient {
    async fn get_tx(&self, txid: elements::Txid) -> Result<elements::Transaction, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.get_tx(txid).await,
            AnyClient::Esplora(client) => client.get_tx(txid).await,
            AnyClient::Waterfalls(client) => client.get_tx(txid).await,
        }
    }

    async fn get_address_utxo(
        &self,
        address: &elements::Address,
    ) -> Result<Option<(elements::OutPoint, elements::TxOut)>, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.get_address_utxo(address).await,
            AnyClient::Esplora(client) => client.get_address_utxo(address).await,
            AnyClient::Waterfalls(client) => client.get_address_utxo(address).await,
        }
    }

    async fn get_genesis_hash(&self) -> Result<elements::BlockHash, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.get_genesis_hash().await,
            AnyClient::Esplora(client) => client.get_genesis_hash().await,
            AnyClient::Waterfalls(client) => client.get_genesis_hash().await,
        }
    }

    async fn broadcast_tx(&self, signed_tx: &elements::Transaction) -> Result<String, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.broadcast_tx(signed_tx).await,
            AnyClient::Esplora(client) => client.broadcast_tx(signed_tx).await,
            AnyClient::Waterfalls(client) => client.broadcast_tx(signed_tx).await,
        }
    }

    fn network(&self) -> LiquidChain {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.network(),
            AnyClient::Esplora(client) => client.network(),
            AnyClient::Waterfalls(client) => client.network(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use boltz_client::bitcoin;
    use boltz_client::network::{BitcoinChain, BitcoinClient, LiquidChain};
    use lwk_wollet::elements::hex::FromHex;

    use super::*;

    #[test]
    fn elements_version_boundary_preserves_transaction_and_output() {
        let output = lwk_elements::TxOut {
            asset: lwk_elements::confidential::Asset::Explicit(
                "0000000000000000000000000000000000000000000000000000000000000000"
                    .parse()
                    .unwrap(),
            ),
            value: lwk_elements::confidential::Value::Explicit(42),
            nonce: lwk_elements::confidential::Nonce::Null,
            script_pubkey: lwk_elements::Script::from(vec![0x51]),
            witness: lwk_elements::TxOutWitness::empty(),
        };
        let tx = lwk_elements::Transaction {
            version: 2,
            lock_time: lwk_elements::LockTime::ZERO,
            input: Vec::new(),
            output: vec![output.clone()],
        };

        let converted = boltz_tx(&tx).unwrap();
        assert_eq!(lwk_tx(&converted).unwrap(), tx);

        let outpoint = lwk_elements::OutPoint {
            txid: tx.txid(),
            vout: 0,
        };
        let (converted_outpoint, converted_output) = boltz_utxo(outpoint, &output).unwrap();
        assert_eq!(
            converted_outpoint.txid.to_string(),
            outpoint.txid.to_string()
        );
        assert_eq!(converted_outpoint.vout, outpoint.vout);
        assert_eq!(
            elements::encode::serialize(&converted_output),
            lwk_elements::encode::serialize(&output)
        );
        assert_eq!(
            converted_output.witness.rangeproof_len(),
            output.witness.rangeproof_len()
        );
    }

    #[test]
    fn elements_version_boundary_preserves_confidential_witness() {
        let hex = include_str!("../../../lwk_wollet/tests/data/usdt-issuance-tx.hex").trim();
        let bytes = Vec::<u8>::from_hex(hex).unwrap();
        let tx: lwk_elements::Transaction = lwk_elements::encode::deserialize(&bytes).unwrap();
        let converted = boltz_tx(&tx).unwrap();
        assert_eq!(lwk_tx(&converted).unwrap(), tx);

        let (index, output) = tx
            .output
            .iter()
            .enumerate()
            .find(|(_, output)| !output.witness.is_empty())
            .unwrap();
        let outpoint = lwk_elements::OutPoint {
            txid: tx.txid(),
            vout: index as u32,
        };
        let (_, converted_output) = boltz_utxo(outpoint, output).unwrap();
        assert_eq!(
            elements::encode::serialize(&converted_output.witness),
            lwk_elements::encode::serialize(&output.witness)
        );
        assert_eq!(
            converted_output.witness.rangeproof_len(),
            output.witness.rangeproof_len()
        );
        assert_eq!(
            converted_output.witness.surjectionproof_len(),
            output.witness.surjectionproof_len()
        );
    }

    struct MockLiquidClient {
        attempts: AtomicUsize,
        succeed_after: usize,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl LiquidClient for MockLiquidClient {
        async fn get_address_utxo(
            &self,
            _address: &elements::Address,
        ) -> Result<Option<(elements::OutPoint, elements::TxOut)>, Error> {
            unimplemented!()
        }

        async fn get_genesis_hash(&self) -> Result<elements::BlockHash, Error> {
            unimplemented!()
        }

        async fn get_tx(&self, _txid: elements::Txid) -> Result<elements::Transaction, Error> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            if attempt >= self.succeed_after {
                Ok(elements::Transaction {
                    version: 2,
                    lock_time: elements::LockTime::ZERO,
                    input: Vec::new(),
                    output: Vec::new(),
                })
            } else {
                Err(Error::Protocol("not found".to_string()))
            }
        }

        async fn broadcast_tx(&self, _signed_tx: &elements::Transaction) -> Result<String, Error> {
            unimplemented!()
        }

        fn network(&self) -> LiquidChain {
            LiquidChain::Liquid
        }
    }

    struct MockBitcoinClient {
        attempts: AtomicUsize,
        succeed_after: usize,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl BitcoinClient for MockBitcoinClient {
        async fn get_address_balance(
            &self,
            _address: &bitcoin::Address,
        ) -> Result<(u64, i64), Error> {
            unimplemented!()
        }

        async fn get_address_utxos(
            &self,
            _address: &bitcoin::Address,
        ) -> Result<Vec<(bitcoin::OutPoint, bitcoin::TxOut)>, Error> {
            unimplemented!()
        }

        async fn get_tx(&self, _txid: bitcoin::Txid) -> Result<bitcoin::Transaction, Error> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
            if attempt >= self.succeed_after {
                Ok(bitcoin::Transaction {
                    version: bitcoin::transaction::Version::TWO,
                    lock_time: bitcoin::absolute::LockTime::ZERO,
                    input: Vec::new(),
                    output: Vec::new(),
                })
            } else {
                Err(Error::Protocol("not found".to_string()))
            }
        }

        async fn broadcast_tx(
            &self,
            _signed_tx: &bitcoin::Transaction,
        ) -> Result<bitcoin::Txid, Error> {
            unimplemented!()
        }

        fn network(&self) -> BitcoinChain {
            BitcoinChain::Bitcoin
        }
    }

    #[tokio::test]
    async fn wait_for_liquid_tx_retries_until_success() {
        let client = MockLiquidClient {
            attempts: AtomicUsize::new(0),
            succeed_after: 2,
        };
        let txid = "0000000000000000000000000000000000000000000000000000000000000000"
            .parse()
            .unwrap();
        let result = wait_for_tx(
            txid,
            Duration::from_millis(50),
            Duration::from_millis(1),
            || client.get_tx(txid),
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(client.attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn wait_for_bitcoin_tx_times_out() {
        let client = MockBitcoinClient {
            attempts: AtomicUsize::new(0),
            succeed_after: usize::MAX,
        };
        let txid = "0000000000000000000000000000000000000000000000000000000000000000"
            .parse()
            .unwrap();
        let result = wait_for_tx(
            txid,
            Duration::from_millis(5),
            Duration::from_millis(1),
            || client.get_tx(txid),
        )
        .await;

        assert!(matches!(result, Err(Error::Protocol(_))));
        assert!(client.attempts.load(Ordering::SeqCst) >= 1);
    }
}
