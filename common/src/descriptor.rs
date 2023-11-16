use std::str::FromStr;

use elements::bitcoin::bip32::DerivationPath;

use crate::Signer;

// TODO impl error handling
pub fn singlesig_desc<S: Signer>(
    signer: &S,
    script_variant: Singlesig,
    blinding_variant: BlindingKeyVariant,
) -> Result<String, String> {
    let (prefix, path, suffix) = match script_variant {
        Singlesig::Wpkh => ("elwpkh", "84h/1h/0h", ""),
        Singlesig::ShWpkh => ("elsh(wpkh", "49h/1h/0h", ")"),
    };

    let master = signer
        .derive_xpub(&DerivationPath::master())
        .map_err(|e| format!("{:?}", e))?;
    let fingerprint = master.fingerprint();

    let xpub = signer
        .derive_xpub(
            &DerivationPath::from_str(&format!("m/{path}")).map_err(|e| format!("{:?}", e))?,
        )
        .map_err(|e| format!("{:?}", e))?;

    let blinding_key = match blinding_variant {
        BlindingKeyVariant::Slip77 => format!(
            "slip77({})",
            signer.slip77().map_err(|e| format!("{:?}", e))?
        ),
    };

    // m / purpose' / coin_type' / account' / change / address_index
    Ok(format!(
        "ct({blinding_key},{prefix}([{fingerprint}/{path}]{xpub}/<0;1>/*){suffix})"
    ))
}

pub enum Singlesig {
    /// as defined by bip84
    Wpkh,

    // as defined by bip49
    ShWpkh,
}

pub enum BlindingKeyVariant {
    Slip77,
}
