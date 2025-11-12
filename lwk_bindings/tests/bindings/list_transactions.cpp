#include <cassert>
#include "lwk.hpp"

int main() {
    auto network = lwk::Network::testnet();
    std::cout << "Network: " << network->to_string() << std::endl;
    assert(network->to_string() == "LiquidTestnet");

    auto mnemonic = lwk::Mnemonic::init("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
    std::cout << "Mnemonic: " << mnemonic->to_string() << std::endl;
    assert(mnemonic->to_string() == "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");

    auto electrum = network->default_electrum_client();
    electrum->ping();

    auto signer = lwk::Signer::init(mnemonic, network);
    auto desc = signer->wpkh_slip77_descriptor();
    std::cout << "Descriptor: " << desc->to_string() << std::endl;
    assert(desc->to_string() == "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d");

    auto wollet = lwk::Wollet::init(network, desc, std::nullopt);

    auto update = electrum->full_scan(wollet);
    if (update) {
        wollet->apply_update(update);
    }

    auto txs = wollet->transactions();
    assert(txs.size() >= 164);
    std::cout << "Number of transactions: " << txs.size() << std::endl;

    for(auto& tx : txs) {
        std::cout << "Transaction ID: " << tx->txid()->to_string() << std::endl;
    }

    return 0;
}