extern crate lwk_wollet;

use lwk_wollet::{
    full_scan_with_electrum_client, ElectrumClient, ElementsNetwork, Wollet, WolletDescriptor,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This example creates a testnet watch only wallet from a CT descriptor
    // and prints a list of its transactions.
    // Run this example with cargo:
    // cargo run --example list_transactions

    let desc = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";

    // Parse the descriptor and create the watch only wallet
    let descriptor: WolletDescriptor = desc.parse()?;
    let mut wollet = Wollet::without_persist(ElementsNetwork::LiquidTestnet, descriptor)?;

    // Sync the wallet using an Electrum client
    let electrum_url = "ssl://elements-testnet.blockstream.info:50002".parse()?;
    let mut electrum_client = ElectrumClient::new(&electrum_url)?;
    full_scan_with_electrum_client(&mut wollet, &mut electrum_client)?;

    // Print a summary of the wallet transactions
    for tx in wollet.transactions()?.into_iter().rev() {
        println!("TXID: {}", tx.txid);
        for (asset, amount) in tx.balance.as_ref() {
            if *amount > 0 {
                println!(" * received: {amount} of asset {asset}");
            } else {
                println!(" * sent:     {} of asset {asset}", -amount);
            }
        }
    }
    Ok(())
}
