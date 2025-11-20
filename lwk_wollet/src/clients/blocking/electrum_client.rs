use crate::clients::electrum_url::ElectrumUrl;
use crate::store::Height;
use crate::Error;
use crate::History;

use electrum_client::ScriptStatus;
use electrum_client::{Client, ConfigBuilder, ElectrumApi, GetHistoryRes};
use elements::encode::deserialize as elements_deserialize;
use elements::encode::serialize as elements_serialize;
use elements::Address;
use elements::{bitcoin, BlockHash, BlockHeader, Script, Transaction, Txid};
use std::collections::HashMap;
use std::fmt::Debug;

use super::BlockchainBackend;

/// A client to issue TCP requests to an electrum server.
pub struct ElectrumClient {
    client: Client,

    tip: BlockHeader,

    script_status: HashMap<Script, ScriptStatus>,
}

impl Debug for ElectrumClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElectrumClient")
            .field("tip", &self.tip)
            .finish()
    }
}

/// Options for the [`ElectrumClient::with_options()`] method.
#[derive(Default)]
pub struct ElectrumOptions {
    /// The timeout for the Electrum client.
    pub timeout: Option<u8>,
}

impl ElectrumClient {
    /// Creates an Electrum client with default options
    pub fn new(url: &ElectrumUrl) -> Result<Self, Error> {
        Self::with_options(url, ElectrumOptions::default())
    }

    /// Creates an Electrum client specifying non default options like timeout
    pub fn with_options(url: &ElectrumUrl, options: ElectrumOptions) -> Result<Self, Error> {
        let client = url.build_client(&options)?;
        let header = client.block_headers_subscribe_raw()?;
        let tip: BlockHeader = elements_deserialize(&header.header)?;

        Ok(Self {
            client,
            tip,
            script_status: HashMap::new(),
        })
    }

    /// Return the status of an address as defined by the electrum protocol
    ///
    /// The status is function of the transaction ids where this address appears and the height of
    /// the block containing when it is confirmed. Unconfirmed transactions use a negative height,
    /// so the status change when they are confirmed.
    pub fn address_status(&mut self, address: &Address) -> Result<Option<ScriptStatus>, Error> {
        let elements_script = address.script_pubkey();
        let bitcoin_script = bitcoin::ScriptBuf::from(elements_script.to_bytes());

        let val = match self.client.script_subscribe(&bitcoin_script) {
            Ok(val) => val,
            Err(electrum_client::Error::AlreadySubscribed(_)) => {
                self.client.script_get_history(&bitcoin_script)?; // it seems it must be called, otherwise the server don't update the status
                self.client.script_pop(&bitcoin_script)?
            }
            Err(e) => return Err(e.into()),
        };

        if let Some(val) = val {
            self.script_status.insert(elements_script.clone(), val);
        }
        Ok(self.script_status.get(&elements_script).cloned())
    }

    /// Ping the Electrum server
    pub fn ping(&self) -> Result<(), Error> {
        Ok(self.client.ping()?)
    }
}
impl BlockchainBackend for ElectrumClient {
    fn tip(&mut self) -> Result<BlockHeader, Error> {
        let mut popped_header = None;
        while let Some(header) = self.client.block_headers_pop_raw()? {
            popped_header = Some(header)
        }

        match popped_header {
            Some(header) => {
                let tip: BlockHeader = elements_deserialize(&header.header)?;
                self.tip = tip;
            }
            None => {
                // https://github.com/bitcoindevkit/rust-electrum-client/issues/124
                // It might be that the client has reconnected and subscriptions don't persist
                // across connections. Calling `client.ping()` won't help here because the
                // successful retry will prevent us knowing about the reconnect.
                if let Ok(header) = self.client.block_headers_subscribe_raw() {
                    let tip: BlockHeader = elements_deserialize(&header.header)?;
                    self.tip = tip;
                }
            }
        }

        Ok(self.tip.clone())
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        // TODO: check that the transaction contains some signatures

        let txid = self
            .client
            .transaction_broadcast_raw(&elements_serialize(tx))?;
        Ok(Txid::from_raw_hash(txid.to_raw_hash()))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
        let txids: Vec<bitcoin::Txid> = txids
            .iter()
            .map(|t| bitcoin::Txid::from_raw_hash(t.to_raw_hash()))
            .collect();

        let mut result = vec![];
        for tx in self.client.batch_transaction_get_raw(&txids)? {
            let tx: Transaction = elements::encode::deserialize(&tx)?;
            result.push(tx);
        }
        Ok(result)
    }

    fn get_headers(
        &self,
        heights: &[Height],
        _: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<BlockHeader>, Error> {
        let mut result = vec![];
        for header in self.client.batch_block_header_raw(heights)? {
            let header: BlockHeader = elements::encode::deserialize(&header)?;
            result.push(header);
        }
        Ok(result)
    }

    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        let scripts: Vec<&bitcoin::Script> = scripts
            .iter()
            .map(|t| bitcoin::Script::from_bytes(t.as_bytes()))
            .collect();

        Ok(self
            .client
            .batch_script_get_history(&scripts)?
            .into_iter()
            .map(|e| e.into_iter().map(Into::into).collect())
            .collect())
    }
}

impl From<GetHistoryRes> for History {
    fn from(value: GetHistoryRes) -> Self {
        History {
            txid: Txid::from_raw_hash(value.tx_hash.to_raw_hash()),
            height: value.height,
            block_hash: None,
            block_timestamp: None,
            v: 0,
        }
    }
}

impl ElectrumUrl {
    /// Build an Electrum client from the url and options
    pub fn build_client(&self, options: &ElectrumOptions) -> Result<Client, Error> {
        let builder = ConfigBuilder::new();
        let (url, builder) = match self {
            ElectrumUrl::Tls(url, validate) => {
                (format!("ssl://{url}"), builder.validate_domain(*validate))
            }
            ElectrumUrl::Plaintext(url) => (format!("tcp://{url}"), builder),
        };
        let builder = builder.timeout(options.timeout);
        Ok(Client::from_config(&url, builder.build())?)
    }
}

#[cfg(test)]
mod tests {
    use super::ElectrumUrl;
    use crate::UrlError;

    fn check_url(url: &str, url_no_scheme: &str, tls: bool, validate_domain: bool) {
        let electrum_url: ElectrumUrl = url.parse().unwrap();
        let url_from_new = ElectrumUrl::new(url_no_scheme, tls, validate_domain).unwrap();
        assert_eq!(electrum_url, url_from_new);
        assert_eq!(electrum_url.to_string(), url);
    }

    #[test]
    fn test_electrum_url() {
        check_url(
            "ssl://blockstream.info:666",
            "blockstream.info:666",
            true,
            true,
        );

        check_url(
            "tcp://blockstream.info:666",
            "blockstream.info:666",
            false,
            false,
        );

        check_url("tcp://1.1.1.1:666", "1.1.1.1:666", false, false);

        check_url(
            "tcp://mrrxtq6tjpbnbm7vh5jt6mpjctn7ggyfy5wegvbeff3x7jrznqawlmid.onion:666",
            "mrrxtq6tjpbnbm7vh5jt6mpjctn7ggyfy5wegvbeff3x7jrznqawlmid.onion:666",
            false,
            false,
        );

        let url_result: Result<ElectrumUrl, UrlError> = "ssl://1.1.1.1:666".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "Cannot specify `ssl` scheme without a domain"
        );

        let url_result: Result<ElectrumUrl, UrlError> = "http://blockstream.info".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "Invalid schema `http` supported ones are `ssl` or `tcp`"
        );

        let url_result: Result<ElectrumUrl, UrlError> = "tcp://blockstream.info".parse();
        assert_eq!(url_result.unwrap_err().to_string(), "Port is missing");

        let url_result: Result<ElectrumUrl, UrlError> = "mailto:rms@example.net".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "Invalid schema `mailto` supported ones are `ssl` or `tcp`"
        );

        let url_result: Result<ElectrumUrl, UrlError> = "xxx".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "relative URL without a base"
        );
    }

    #[test]
    fn test_electrum_url_new() {
        let err = ElectrumUrl::new("example.com", false, true)
            .unwrap_err()
            .to_string();
        assert_eq!(err, "Cannot validate the domain without tls");

        let err = ElectrumUrl::new("ssl://example.com", false, false)
            .unwrap_err()
            .to_string();
        assert_eq!(err, "Don't specify the scheme in the url");
    }
}
