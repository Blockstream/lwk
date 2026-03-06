use crate::{Chain, Error, WolletDescriptor, EC};
use elements::bitcoin::bip32::{DerivationPath, Xpriv, Xpub};
use elements::hex::ToHex;
use elements_miniscript::slip77::MasterBlindingKey;
use std::path::Path;

impl WolletDescriptor {
    /// Construct a WolletDescriptor from the output of Elements RPC `dumpwallet`
    ///
    /// This does not guarantee to map all addresses.
    ///
    /// **Unstable**: This API may change without notice.
    #[doc(hidden)]
    pub fn from_dumpwallet<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        let master_xprv: Xpriv = content
            .lines()
            .find_map(|l| l.strip_prefix("# extended private masterkey: "))
            .ok_or_else(|| Error::Generic("missing master xprv".into()))?
            .trim()
            .parse()?;
        let mbk: MasterBlindingKey = content
            .lines()
            .find_map(|l| l.strip_prefix("# Master private blinding key: "))
            .ok_or_else(|| Error::Generic("missing master blinding key".into()))?
            .trim()
            .parse()?;
        // TODO: parse the other lines and check they're consistent
        // TODO: get the last reserved index
        let max_index = 100;

        let mut v = vec![];
        for is_sh in [false, true] {
            for chain in 0..=1 {
                for index in 0..max_index {
                    let path = format!("0h/{chain}h/{index}h");
                    let path: DerivationPath = path.parse()?;
                    let xprv = master_xprv.derive_priv(&EC, &path)?;
                    let xpub = Xpub::from_priv(&EC, &xprv);
                    let desc = if is_sh {
                        format!("ct(slip77({mbk}),elsh(wpkh({xpub})))")
                    } else {
                        format!("ct(slip77({mbk}),elwpkh({xpub}))")
                    };
                    // note: passing through the desc is suboptimal
                    let wd: WolletDescriptor = desc.parse()?;
                    // descriptor with no wildcards, following params are unused
                    let (ext_int, index) = (Chain::External, 0);
                    let spk = wd.script_pubkey(ext_int, index)?;
                    let pbk = lwk_common::derive_blinding_key(wd.ct_descriptor()?, &spk)
                        .ok_or_else(|| Error::Generic("cannot derive blinding key".into()))?;
                    v.push(format!("{}:{}", pbk.display_secret(), spk.to_hex()));
                }
            }
        }
        let desc = v.join(",");
        desc.parse()
    }
}
