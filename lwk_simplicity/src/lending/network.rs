use lwk_common::Network;
use simplex::provider::SimplicityNetwork;

pub fn to_simplicity_network(network: Network) -> SimplicityNetwork {
    match network {
        Network::Liquid => SimplicityNetwork::Liquid,
        Network::TestnetLiquid => SimplicityNetwork::LiquidTestnet,
        Network::CustomElements(_) => SimplicityNetwork::ElementsRegtest {
            policy_asset: *network.policy_asset(),
        },
    }
}
