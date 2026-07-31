//! Reads block-input prevout scripts from Esplora transaction listings.

use std::collections::HashMap;

use crate::elements::{OutPoint, Script, Txid};

/// A `/block/{hash}/txs` page.
#[derive(serde::Deserialize)]
pub(crate) struct BlockTxsPage(Vec<BlockTx>);

#[derive(serde::Deserialize)]
struct BlockTx {
    vin: Vec<BlockTxIn>,
}

#[derive(serde::Deserialize)]
struct BlockTxIn {
    txid: Txid,
    vout: u32,
    /// Absent for coinbase and peg-in inputs.
    prevout: Option<BlockPrevout>,
    #[serde(default)]
    is_coinbase: bool,
    #[serde(default)]
    is_pegin: bool,
}

#[derive(serde::Deserialize)]
struct BlockPrevout {
    /// Hex-encoded scriptPubKey of the output being spent.
    scriptpubkey: String,
}

impl BlockTxsPage {
    /// Number of transactions in this page.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// Adds decodable non-coinbase, non-peg-in prevout scripts to `out`.
    pub(crate) fn collect_into(&self, out: &mut HashMap<OutPoint, Script>) {
        for tx in &self.0 {
            for input in &tx.vin {
                if input.is_coinbase || input.is_pegin {
                    continue;
                }
                let Some(prevout) = &input.prevout else {
                    continue;
                };
                let Ok(script) = prevout.scriptpubkey.parse::<Script>() else {
                    continue;
                };
                out.insert(
                    OutPoint {
                        txid: input.txid,
                        vout: input.vout,
                    },
                    script,
                );
            }
        }
    }
}

/// Esplora returns block transactions in pages of this size.
pub(crate) const BLOCK_TXS_PAGE_SIZE: usize = 25;

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a real `/block/{hash}/txs` response on Liquid testnet: a coinbase
    /// (null prevout) and an ordinary p2wpkh spend carrying its prevout script.
    const PAGE: &str = r#"[
      {
        "txid": "0226949ff2a9f9b6c7f0a93aef2db6c1bbd0ef95ce4881077013d8d2fb631cda",
        "vin": [{
          "txid": "0000000000000000000000000000000000000000000000000000000000000000",
          "vout": 4294967295,
          "prevout": null,
          "is_coinbase": true,
          "is_pegin": false
        }]
      },
      {
        "txid": "3e8dc4ae3c6d58c8af69137d5d93354ee1995580f80274f2f9d9fed90539dcc5",
        "vin": [{
          "txid": "2ff1893242d1bfcd999ef5d531768e5f8a26f36f04d0c06c7e24b6dae548f4a4",
          "vout": 0,
          "prevout": {
            "scriptpubkey": "00142d0f8ce38a5d3e8d97f415b4f28906b9cc8fdec7",
            "scriptpubkey_type": "v0_p2wpkh"
          },
          "is_coinbase": false,
          "is_pegin": false
        }]
      }
    ]"#;

    /// The whole point of this module: the spent script arrives inline, so it must be
    /// recovered without any further request.
    #[test]
    fn prevout_scripts_are_read_from_the_listing() {
        let page: BlockTxsPage = serde_json::from_str(PAGE).unwrap();
        assert_eq!(
            page.len(),
            2,
            "both transactions must be counted for paging"
        );

        let mut prevouts = HashMap::new();
        page.collect_into(&mut prevouts);

        let op = OutPoint {
            txid: "2ff1893242d1bfcd999ef5d531768e5f8a26f36f04d0c06c7e24b6dae548f4a4"
                .parse()
                .unwrap(),
            vout: 0,
        };
        let script = prevouts.get(&op).expect("the spent script must be present");
        assert_eq!(
            crate::elements::hex::ToHex::to_hex(script.as_bytes()),
            "00142d0f8ce38a5d3e8d97f415b4f28906b9cc8fdec7",
            "the script must round-trip byte-for-byte, or tweaks are computed over the wrong key"
        );

        // Keyed by the spent outpoint, never by the spending transaction.
        assert_eq!(prevouts.len(), 1, "the coinbase must contribute nothing");
    }

    /// A peg-in's `previous_output` names a Bitcoin transaction, and Esplora sends its
    /// prevout as null. It contributes no key to the tweak, so it must be skipped rather
    /// than recorded as a missing script.
    #[test]
    fn pegin_inputs_are_skipped() {
        let json = r#"[{
          "txid": "3e8dc4ae3c6d58c8af69137d5d93354ee1995580f80274f2f9d9fed90539dcc5",
          "vin": [{
            "txid": "0c52d2526a5c9f00e9fb74afd15dd3caaf17c823159a514f929ae25193a43a52",
            "vout": 0,
            "prevout": null,
            "is_coinbase": false,
            "is_pegin": true
          }]
        }]"#;

        let page: BlockTxsPage = serde_json::from_str(json).unwrap();
        let mut prevouts = HashMap::new();
        page.collect_into(&mut prevouts);
        assert!(prevouts.is_empty());
    }

    /// Esplora sends many more fields than these structs name; binding to all of them
    /// would break the client whenever upstream adds one.
    #[test]
    fn unknown_fields_are_tolerated() {
        let json = r#"[{
          "txid": "3e8dc4ae3c6d58c8af69137d5d93354ee1995580f80274f2f9d9fed90539dcc5",
          "version": 2,
          "locktime": 1224565,
          "size": 21967,
          "status": {"confirmed": true, "block_height": 1224566},
          "vin": [{
            "txid": "2ff1893242d1bfcd999ef5d531768e5f8a26f36f04d0c06c7e24b6dae548f4a4",
            "vout": 0,
            "scriptsig": "",
            "witness": ["aabb"],
            "sequence": 4294967293,
            "prevout": {
              "scriptpubkey": "00142d0f8ce38a5d3e8d97f415b4f28906b9cc8fdec7",
              "valuecommitment": "097bc0ac90c7c0e86a7b0f27c11da235c823e9814419fa5b014a4040a5e3c2b3c8"
            },
            "is_coinbase": false,
            "is_pegin": false
          }],
          "vout": [{"scriptpubkey": "6a", "value": 0}]
        }]"#;

        let page: BlockTxsPage = serde_json::from_str(json).unwrap();
        let mut prevouts = HashMap::new();
        page.collect_into(&mut prevouts);
        assert_eq!(prevouts.len(), 1);
    }
}
