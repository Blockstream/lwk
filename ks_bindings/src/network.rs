use crate::types::AssetId;

/// Possible valid networks
#[derive(uniffi::Enum, Debug)]
pub enum ElementsNetwork {
    Liquid,
    LiquidTestnet,
    ElementsRegtest { policy_asset: AssetId },
}
impl From<ElementsNetwork> for wollet::ElementsNetwork {
    fn from(value: ElementsNetwork) -> Self {
        match value {
            ElementsNetwork::Liquid => wollet::ElementsNetwork::Liquid,
            ElementsNetwork::LiquidTestnet => wollet::ElementsNetwork::LiquidTestnet,
            ElementsNetwork::ElementsRegtest { policy_asset } => {
                wollet::ElementsNetwork::ElementsRegtest {
                    policy_asset: policy_asset.into(),
                }
            }
        }
    }
}

impl From<wollet::ElementsNetwork> for ElementsNetwork {
    fn from(value: wollet::ElementsNetwork) -> Self {
        match value {
            wollet::ElementsNetwork::Liquid => ElementsNetwork::Liquid,
            wollet::ElementsNetwork::LiquidTestnet => ElementsNetwork::LiquidTestnet,
            wollet::ElementsNetwork::ElementsRegtest { policy_asset } => {
                ElementsNetwork::ElementsRegtest {
                    policy_asset: policy_asset.into(),
                }
            }
        }
    }
}

impl ElementsNetwork {
    pub fn electrum_url(&self) -> &str {
        match self {
            ElementsNetwork::Liquid => "blockstream.info:995",
            ElementsNetwork::LiquidTestnet => "blockstream.info:465",
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => "127.0.0.1:50002",
        }
    }
}

#[uniffi::export]
fn network_to_string(network: &ElementsNetwork) -> String {
    // it looks `#[uniffi::export(Display)]` works only on struct not on enum
    format!("{:?}", network)
}

#[cfg(test)]
mod tests {
    use crate::network::network_to_string;

    use super::ElementsNetwork;

    #[test]
    fn network() {
        let policy_asset = elements::AssetId::default();
        for n in [
            wollet::ElementsNetwork::Liquid,
            wollet::ElementsNetwork::LiquidTestnet,
            wollet::ElementsNetwork::ElementsRegtest { policy_asset },
        ] {
            let n2: ElementsNetwork = n.clone().into();
            assert!(
                ["Liquid", "LiquidTestnet", "ElementsRegtest { policy_asset: AssetId { inner: 0000000000000000000000000000000000000000000000000000000000000000 } }"]
                    .contains(&network_to_string(&n2).as_str())
            );
            assert!([
                "blockstream.info:995",
                "blockstream.info:465",
                "127.0.0.1:50002"
            ]
            .contains(&n2.electrum_url()));

            let n3: wollet::ElementsNetwork = n2.into();
            assert_eq!(n, n3);
        }
    }
}
