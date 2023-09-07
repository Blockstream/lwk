use elements::bitcoin::PublicKey;
use elements::{Address, AddressParams, Script};

// The following scripts are always using regtest network,
// it is always ok because I am not interested in the address just in the script

pub fn p2shwpkh_script(pk: &PublicKey) -> Script {
    Address::p2shwpkh(pk, None, &AddressParams::ELEMENTS).script_pubkey()
}
