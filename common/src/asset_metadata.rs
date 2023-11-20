use std::io::Cursor;

use elements::{
    encode::{self, Decodable, Encodable},
    pset::{raw::ProprietaryKey, PartiallySignedTransaction},
    AssetId, OutPoint,
};

/// TODO move to rust-elements
/// Contains extension to add and retrieve from the PSET contract information related to an asset
pub trait PsetExt {
    /// Add contract information to the PSET, returns None if it wasn't present or Some with the old
    /// data if already in the PSET (data to be deserialized in AssetMetadata)
    fn add_asset_metadata(
        &mut self,
        asset_id: AssetId,
        asset_meta: &AssetMetadata,
    ) -> Option<Result<AssetMetadata, encode::Error>>;

    /// Get contract information from the PSET, returns None if there are no information regarding
    /// the given asset_id in the PSET
    fn get_asset_metadata(&self, asset_id: AssetId)
        -> Option<Result<AssetMetadata, encode::Error>>;
}

#[derive(Debug, PartialEq, Eq)]
/// Asset metadata, the contract and the outpoint used to issue the asset
pub struct AssetMetadata {
    pub contract: String,
    pub issuance_prevout: OutPoint,
}

impl PsetExt for PartiallySignedTransaction {
    fn add_asset_metadata(
        &mut self,
        asset_id: AssetId,
        asset_meta: &AssetMetadata,
    ) -> Option<Result<AssetMetadata, encode::Error>> {
        let key = prop_key(asset_id);
        self.global
            .proprietary
            .insert(key, asset_meta.serialize())
            .map(|old| AssetMetadata::deserialize(&old))
    }

    fn get_asset_metadata(
        &self,
        asset_id: AssetId,
    ) -> Option<Result<AssetMetadata, encode::Error>> {
        let key = prop_key(asset_id);

        self.global
            .proprietary
            .get(&key)
            .map(|data| AssetMetadata::deserialize(data))
    }
}

fn prop_key(asset_id: AssetId) -> ProprietaryKey {
    // equivalent to asset_tag
    let mut key = Vec::with_capacity(32);
    asset_id
        .consensus_encode(&mut key)
        .expect("vec doesn't err");

    ProprietaryKey {
        prefix: String::from("pset_hww").into_bytes(),
        subtype: 0x00, // FIXME, this subtype is wrong, update before release, ELIP100
        key,
    }
}

impl AssetMetadata {
    /// Returns the contract as string containing a json
    pub fn contract(&self) -> &str {
        &self.contract
    }

    /// Returns the issuance prevout where the asset has been issued
    pub fn issuance_prevout(&self) -> OutPoint {
        self.issuance_prevout
    }

    /// Serialize this metadata as defined by ELIP0100
    ///
    /// `<compact size uint contractLen><contract><32-byte prevoutTxid><32-bit little endian uint prevoutIndex>`
    pub fn serialize(&self) -> Vec<u8> {
        let mut result = vec![];

        self.contract
            .as_bytes()
            .to_vec()
            .consensus_encode(&mut result)
            .expect("vec doesn't err"); // TODO improve efficiency avoding to_vec

        self.issuance_prevout
            .consensus_encode(&mut result)
            .expect("vec doesn't err");

        result
    }

    pub fn deserialize(data: &[u8]) -> Result<AssetMetadata, encode::Error> {
        let mut cursor = Cursor::new(data);
        let str_bytes = Vec::<u8>::consensus_decode(&mut cursor)?;

        let contract = String::from_utf8(str_bytes).map_err(|_| {
            encode::Error::ParseFailed("utf8 conversion fail on the contract string")
        })?;

        let issuance_prevout = OutPoint::consensus_decode(&mut cursor)?;

        Ok(AssetMetadata {
            contract,
            issuance_prevout,
        })
    }
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use elements::{
        encode::Encodable, hex::ToHex, pset::PartiallySignedTransaction, AssetId, ContractHash,
    };

    use super::{prop_key, AssetMetadata, PsetExt};

    const CONTRACT_HASH: &str = "3c7f0a53c2ff5b99590620d7f6604a7a3a7bfbaaa6aa61f7bfc7833ca03cde82";
    const VALID_CONTRACT: &str = r#"{"entity":{"domain":"tether.to"},"issuer_pubkey":"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904","name":"Tether USD","precision":8,"ticker":"USDt","version":0}"#;
    const ISSUANCE_PREVOUT: &str =
        "9596d259270ef5bac0020435e6d859aea633409483ba64e232b8ba04ce288668:0";
    const ASSET_ID: &str = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";

    // TODO At the moment (23 Oct 2023) there are no official test vectors in the ELIP0100 so don't
    // base anything on this value yet
    #[test]
    fn asset_metadata_roundtrip() {
        let a = AssetMetadata {
            contract: VALID_CONTRACT.to_string(),
            issuance_prevout: ISSUANCE_PREVOUT.parse().unwrap(),
        };
        let contract_hash = ContractHash::from_str(CONTRACT_HASH).unwrap();
        assert_eq!(
            ContractHash::from_json_contract(VALID_CONTRACT).unwrap(),
            contract_hash
        );
        let serialized = a.serialize();
        assert_eq!(serialized.to_hex(),"b47b22656e74697479223a7b22646f6d61696e223a227465746865722e746f227d2c226973737565725f7075626b6579223a22303333376363656563306265656130323332656265313463626130313937613966626434356663663265633934363734396465393230653731343334633262393034222c226e616d65223a2254657468657220555344222c22707265636973696f6e223a382c227469636b6572223a2255534474222c2276657273696f6e223a307d688628ce04bab832e264ba83944033a6ae59d8e6350402c0baf50e2759d2969500000000");
        let b = AssetMetadata::deserialize(&serialized).unwrap();
        assert_eq!(a, b);
    }

    // TODO At the moment (23 Oct 2023) there are no official test vectors in the ELIP0100 so don't
    // base anything on this value yet
    #[test]
    #[ignore]
    fn prop_key_serialize() {
        let asset_id = AssetId::from_str(ASSET_ID).unwrap();

        let key = prop_key(asset_id);
        let mut vec = vec![];
        key.consensus_encode(&mut vec).unwrap();
        assert_eq!(vec.to_hex(), "wrong until ELIP100 define the std");
    }

    #[test]
    fn pset_set_get_asset_metadata() {
        let mut pset = PartiallySignedTransaction::new_v2();
        let asset_meta = AssetMetadata {
            contract: VALID_CONTRACT.to_string(),
            issuance_prevout: ISSUANCE_PREVOUT.parse().unwrap(),
        };
        let asset_id = AssetId::from_str(ASSET_ID).unwrap();

        let get = pset.get_asset_metadata(asset_id);
        assert!(get.is_none());

        let old = pset.add_asset_metadata(asset_id, &asset_meta);
        assert!(old.is_none());

        let old = pset
            .add_asset_metadata(asset_id, &asset_meta)
            .unwrap()
            .unwrap();
        assert_eq!(asset_meta, old);

        let get = pset.get_asset_metadata(asset_id).unwrap().unwrap();
        assert_eq!(asset_meta, get);
    }
}
