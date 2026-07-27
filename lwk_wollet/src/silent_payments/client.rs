//! Client for a silent payments index server following the "Tweak Server" model of the
//! [BIP352 index server specification].
//!
//! The wallet asks the server for the tweak data of a block, derives locally the scripts it
//! would be paid to, and downloads a block only when a compact block filter says one of those
//! scripts is in it. No key is ever shared with the server, and the server does not learn
//! which transactions the wallet is interested in.
//!
//! [BIP352 index server specification]: https://github.com/silent-payments/BIP0352-index-server-specification

use elements::bitcoin::bip158::BlockFilter;
use elements::hashes::Hash;
use elements::hex::FromHex;
use elements::{BlockHash, Script};
use serde::Deserialize;

use crate::cache::Height;
use crate::secp256k1::PublicKey;
use crate::Error;

/// Information about a tweak server instance, as returned by `GET /getinfo`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// The version of the server
    pub version: String,

    /// The network the server is indexing
    pub network: String,

    /// The height of the last block the server has indexed
    pub block_height: Height,
}

/// The tweak data of a block, as returned by `GET /tweaks/:blockheight`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTweaks {
    /// The height of the block
    pub height: Height,

    /// The hash of the block, needed to match against its filter
    pub hash: BlockHash,

    /// One tweak per silent payment eligible transaction of the block
    pub tweaks: Vec<PublicKey>,
}

#[derive(Deserialize)]
struct ServerInfoResponse {
    version: String,
    network: String,
    block_height: Height,
}

#[derive(Deserialize)]
struct TweaksResponse {
    height: Height,
    hash: String,
    tweaks: Vec<String>,
}

#[derive(Deserialize)]
struct FilterResponse {
    filter: String,
}

impl TryFrom<TweaksResponse> for BlockTweaks {
    type Error = Error;

    fn try_from(response: TweaksResponse) -> Result<Self, Error> {
        let tweaks = response
            .tweaks
            .iter()
            .map(|tweak| {
                let bytes = Vec::<u8>::from_hex(tweak)?;
                Ok(PublicKey::from_slice(&bytes)?)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(BlockTweaks {
            height: response.height,
            hash: response.hash.parse()?,
            tweaks,
        })
    }
}

/// A client for a silent payments tweak server, see the [module documentation](self)
#[derive(Debug, Clone)]
pub struct TweakServerClient {
    client: reqwest::Client,
    base_url: String,
}

impl TweakServerClient {
    /// Create a client for the server at `base_url`
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let url = format!("{}{path}", self.base_url);
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::EsploraHttpError {
                url,
                status: status.as_u16(),
                body: response.text().await.ok(),
            });
        }
        Ok(response.json().await?)
    }

    /// Basic information about the server, `GET /getinfo`
    pub async fn get_info(&self) -> Result<ServerInfo, Error> {
        let response: ServerInfoResponse = self.get("/getinfo").await?;
        Ok(ServerInfo {
            version: response.version,
            network: response.network,
            block_height: response.block_height,
        })
    }

    /// The tweak data of the block at `height`, `GET /tweaks/:blockheight`
    pub async fn tweaks(&self, height: Height) -> Result<BlockTweaks, Error> {
        let response: TweaksResponse = self.get(&format!("/tweaks/{height}")).await?;
        response.try_into()
    }

    /// The BIP158 basic filter of the block at `height`, `GET /filters/:blockheight`
    ///
    /// A wallet may obtain the same filter from any other source, for instance from an
    /// Elements node run with `-blockfilterindex=basic`.
    pub async fn filter(&self, height: Height) -> Result<BlockFilter, Error> {
        let response: FilterResponse = self.get(&format!("/filters/{height}")).await?;
        Ok(BlockFilter::new(&Vec::<u8>::from_hex(&response.filter)?))
    }
}

/// Whether any of `scripts` is in the block filter.
///
/// A false positive only costs a block download, a false negative would lose a payment, so a
/// filter that cannot be parsed is treated as a match.
pub fn filter_matches(filter: &BlockFilter, block_hash: &BlockHash, scripts: &[Script]) -> bool {
    if scripts.is_empty() {
        return false;
    }
    // BIP158 keys the filter on the block hash bytes, which are the same on Elements
    let block_hash = elements::bitcoin::BlockHash::from_byte_array(block_hash.to_byte_array());
    filter
        .match_any(&block_hash, scripts.iter().map(|s| s.as_bytes()))
        .unwrap_or(true)
}

/// Blocking version of [`TweakServerClient`]
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct BlockingTweakServerClient {
    rt: tokio::runtime::Runtime,
    client: TweakServerClient,
}

#[cfg(not(target_arch = "wasm32"))]
impl BlockingTweakServerClient {
    /// Create a client for the server at `base_url`
    pub fn new(base_url: &str) -> Result<Self, Error> {
        Ok(Self {
            rt: tokio::runtime::Runtime::new()?,
            client: TweakServerClient::new(base_url),
        })
    }

    /// See [`TweakServerClient::get_info`]
    pub fn get_info(&self) -> Result<ServerInfo, Error> {
        self.rt.block_on(self.client.get_info())
    }

    /// See [`TweakServerClient::tweaks`]
    pub fn tweaks(&self, height: Height) -> Result<BlockTweaks, Error> {
        self.rt.block_on(self.client.tweaks(height))
    }

    /// See [`TweakServerClient::filter`]
    pub fn filter(&self, height: Height) -> Result<BlockFilter, Error> {
        self.rt.block_on(self.client.filter(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWEAK: &str = "03398173f560782d934ddf4f5a291c47fd0866d6e26a97a7407b810e1873e34777";
    const HASH: &str = "1aea43f7205bd02c03b081f3c6de4604756bc1a37ae2ca6e34b1936137756870";

    fn client(status: &'static str, body: &'static str) -> TweakServerClient {
        let url = lwk_test_util::serve_http_response(status, "application/json", body, false);
        TweakServerClient::new(&url)
    }

    #[tokio::test]
    async fn get_info() {
        let body =
            r#"{"version":"1.0.0","network":"liquidv1","block_height":3984398,"dust_limit":0}"#;
        let info = client("200 OK", body).get_info().await.unwrap();
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.network, "liquidv1");
        assert_eq!(info.block_height, 3_984_398);
    }

    #[tokio::test]
    async fn tweaks() {
        let body = r#"{
            "height": 100,
            "hash": "1aea43f7205bd02c03b081f3c6de4604756bc1a37ae2ca6e34b1936137756870",
            "dust_limit": 0,
            "filter_spent": 0,
            "tweaks": ["03398173f560782d934ddf4f5a291c47fd0866d6e26a97a7407b810e1873e34777"]
        }"#;
        let tweaks = client("200 OK", body).tweaks(100).await.unwrap();
        assert_eq!(tweaks.height, 100);
        assert_eq!(tweaks.hash.to_string(), HASH);
        assert_eq!(tweaks.tweaks.len(), 1);
        assert_eq!(tweaks.tweaks[0].to_string(), TWEAK);
    }

    /// Most blocks contain no silent payment, this must not be an error
    #[tokio::test]
    async fn block_without_tweaks() {
        let body = r#"{
            "height": 100,
            "hash": "1aea43f7205bd02c03b081f3c6de4604756bc1a37ae2ca6e34b1936137756870",
            "tweaks": []
        }"#;
        let tweaks = client("200 OK", body).tweaks(100).await.unwrap();
        assert!(tweaks.tweaks.is_empty());
    }

    /// The error body of the index server is JSON, it must not be parsed as a success
    #[tokio::test]
    async fn http_error() {
        let body = r#"{"error":{"code":400,"message":"Invalid block height"}}"#;
        let err = client("400 Bad Request", body).tweaks(1).await.unwrap_err();
        assert!(
            matches!(err, Error::EsploraHttpError { status: 400, .. }),
            "{err:?}"
        );
    }

    /// A tweak that is not a public key would make the wallet miss every payment in the block,
    /// so it must be rejected rather than skipped
    #[tokio::test]
    async fn malformed_tweak() {
        let body = r#"{
            "height": 1,
            "hash": "1aea43f7205bd02c03b081f3c6de4604756bc1a37ae2ca6e34b1936137756870",
            "tweaks": ["00"]
        }"#;
        assert!(client("200 OK", body).tweaks(1).await.is_err());
    }
}
