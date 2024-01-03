use crate::AssetId;

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

impl ElementsNetwork {
    pub fn electrum_url(&self) -> &str {
        match self {
            ElementsNetwork::Liquid => "blockstream.info:995",
            ElementsNetwork::LiquidTestnet => "blockstream.info:465",
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => todo!(),
        }
    }
}

#[uniffi::export]
fn network_to_string(network: ElementsNetwork) -> String {
    // it looks `#[uniffi::export(Display)]` works only on struct not on enum
    format!("{:?}", network)
}
