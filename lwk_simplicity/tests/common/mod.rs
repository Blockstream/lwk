use std::str::FromStr;

use elements::Txid;

use lwk_signer::*;
use lwk_test_util::*;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::*;

pub fn generate_signer() -> SwSigner {
    let mnemonic = generate_mnemonic();
    SwSigner::new(&mnemonic, false).unwrap()
}

pub fn electrum_client(env: &TestEnv) -> ElectrumClient {
    let electrum_url = ElectrumUrl::from_str(&env.electrum_url()).unwrap();
    ElectrumClient::new(&electrum_url).unwrap()
}

pub fn sync<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S) {
    let update = client.full_scan(wollet).unwrap();
    if let Some(update) = update {
        wollet.apply_update(update).unwrap();
    }
}

pub fn wait_for_tx<S: BlockchainBackend>(wollet: &mut Wollet, client: &mut S, txid: &Txid) {
    for _ in 0..120 {
        sync(wollet, client);
        let list = wollet.transactions().unwrap();
        if list.iter().any(|e| &e.txid == txid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    panic!("Wallet does not have {txid} in its list");
}
