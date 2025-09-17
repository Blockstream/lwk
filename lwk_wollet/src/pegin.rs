//! Pegin related functions (WIP)
//!
//! A Peg-in is a way to convert bitcoin (BTC) on the mainchain to liquid bitcoin (L-BTC).

use elements::{bitcoin, BlockHeader};

/// Returns the height of the block containing full federation parameters
///
/// For example in liquid only headers with `(height % 20160) == 0` contains full parameters
#[cfg(not(target_arch = "wasm32"))]
fn height_with_fed_peg_script(network: crate::ElementsNetwork, current_tip: u32) -> u32 {
    // GetValidFedpegScripts # function in elements codebase for valid pegin scripts

    (current_tip / network.dynamic_epoch_length()) * network.dynamic_epoch_length()
}

/// Fetch the fed peg script from the header
pub fn fed_peg_script(header: &BlockHeader) -> Option<bitcoin::ScriptBuf> {
    match &header.ext {
        elements::BlockExtData::Proof { .. } => None,
        elements::BlockExtData::Dynafed { current, .. } => current
            .fedpegscript()
            .map(|e| bitcoin::ScriptBuf::from_bytes(e.clone())),
    }
}

// TODO move this in the trait
/// Fetch the last full header, the full header is the header with the fed peg script which is not always present.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_last_full_header<B: crate::clients::blocking::BlockchainBackend>(
    client: &B,
    network: crate::ElementsNetwork,
    current_tip: u32,
) -> Result<BlockHeader, crate::Error> {
    let height = height_with_fed_peg_script(network, current_tip);
    let mut headers = client.get_headers(&[height], &std::collections::HashMap::new())?;
    headers
        .pop()
        .ok_or(crate::Error::Generic("No headers returned".to_string()))
}

#[cfg(test)]
mod test {
    use crate::ElementsNetwork;

    use super::{fed_peg_script, height_with_fed_peg_script};

    // TODO move in test util
    const FED_PEG_SCRIPT: &str = "5b21020e0338c96a8870479f2396c373cc7696ba124e8635d41b0ea581112b678172612102675333a4e4b8fb51d9d4e22fa5a8eaced3fdac8a8cbf9be8c030f75712e6af992102896807d54bc55c24981f24a453c60ad3e8993d693732288068a23df3d9f50d4821029e51a5ef5db3137051de8323b001749932f2ff0d34c82e96a2c2461de96ae56c2102a4e1a9638d46923272c266631d94d36bdb03a64ee0e14c7518e49d2f29bc401021031c41fdbcebe17bec8d49816e00ca1b5ac34766b91c9f2ac37d39c63e5e008afb2103079e252e85abffd3c401a69b087e590a9b86f33f574f08129ccbd3521ecf516b2103111cf405b627e22135b3b3733a4a34aa5723fb0f58379a16d32861bf576b0ec2210318f331b3e5d38156da6633b31929c5b220349859cc9ca3d33fb4e68aa08401742103230dae6b4ac93480aeab26d000841298e3b8f6157028e47b0897c1e025165de121035abff4281ff00660f99ab27bb53e6b33689c2cd8dcd364bc3c90ca5aea0d71a62103bd45cddfacf2083b14310ae4a84e25de61e451637346325222747b157446614c2103cc297026b06c71cbfa52089149157b5ff23de027ac5ab781800a578192d175462103d3bde5d63bdb3a6379b461be64dad45eabff42f758543a9645afd42f6d4248282103ed1e8d5109c9ed66f7941bc53cc71137baa76d50d274bda8d5e8ffbd6e61fe9a5fae736402c00fb269522103aab896d53a8e7d6433137bbba940f9c521e085dd07e60994579b64a6d992cf79210291b7d0b1b692f8f524516ed950872e5da10fb1b808b5a526dedc6fed1cf29807210386aa9372fbab374593466bc5451dc59954e90787f08060964d95c87ef34ca5bb53ae68";

    #[test]
    fn test_height_with_fed_peg_script() {
        assert_eq!(
            height_with_fed_peg_script(ElementsNetwork::Liquid, 2_963_521),
            2_963_520
        );
    }

    #[test]
    fn test_fed_peg_script() {
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let script = fed_peg_script(&header).unwrap();
        assert_eq!(script.to_hex_string(), FED_PEG_SCRIPT);
    }
}
