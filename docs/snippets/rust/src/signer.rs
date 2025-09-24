fn generate_signer() -> anyhow::Result<()> {
    // ANCHOR: generate-signer
    use lwk_signer::{bip39::Mnemonic, SwSigner};

    let mnemonic = Mnemonic::generate(12)?;
    let is_mainnet = false;

    let signer = SwSigner::new(&mnemonic.to_string(), is_mainnet)?;
    // ANCHOR_END: generate-signer

    Ok(())
}
