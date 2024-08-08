use elements::{bitcoin, BlockHeader};

use crate::ElementsNetwork;

/// Returns the height of the block containing full federation parameters
///
/// For example in liquid only headers with `(height % 20160) == 0` contains full parameters
fn height_with_fed_peg_script(network: ElementsNetwork, current_tip: u32) -> u32 {
    // GetValidFedpegScripts # function in elements codebase for valid pegin scripts

    (current_tip / network.dynamic_epoch_length()) * network.dynamic_epoch_length()
}

fn fed_peg_script(header: &BlockHeader) -> Option<bitcoin::ScriptBuf> {
    match &header.ext {
        elements::BlockExtData::Proof { .. } => None,
        elements::BlockExtData::Dynafed { current, .. } => current
            .fedpegscript()
            .map(|e| bitcoin::ScriptBuf::from_bytes(e.clone())),
    }
}

#[cfg(test)]
mod test {
    use crate::ElementsNetwork;

    use super::{fed_peg_script, height_with_fed_peg_script};

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
        let _script = fed_peg_script(&header).unwrap();
    }
}
