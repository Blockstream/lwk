use elements::bitcoin::bip32::Xpriv;
use elements::bitcoin::Network;
use elements::hex::ToHex;
use elements_miniscript::elements::{self};
use rand::{thread_rng, Rng};

pub fn generate_mnemonic() -> String {
    let mut bytes = [0u8; 16];
    thread_rng().fill(&mut bytes);
    bip39::Mnemonic::from_entropy(&bytes).unwrap().to_string()
}

pub fn generate_slip77() -> String {
    let mut bytes = [0u8; 32];
    thread_rng().fill(&mut bytes);
    bytes.to_hex()
}

pub fn generate_view_key() -> String {
    let mut bytes = [0u8; 32];
    thread_rng().fill(&mut bytes);
    bytes.to_hex()
}

pub fn generate_xprv() -> Xpriv {
    let mut seed = [0u8; 16];
    thread_rng().fill(&mut seed);
    Xpriv::new_master(Network::Regtest, &seed).unwrap()
}
