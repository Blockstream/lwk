#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use lwk_wollet::{Update, WolletDescriptor};

const DESCRIPTOR: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
const UPDATE_PREFIX: [u8; 7] = [0x89, 0x61, 0xb8, 0xc8, 0, 0, 0];

fn descriptor() -> &'static WolletDescriptor {
    static DESCRIPTOR_CELL: OnceLock<WolletDescriptor> = OnceLock::new();
    DESCRIPTOR_CELL.get_or_init(|| DESCRIPTOR.parse().expect("valid descriptor"))
}

fuzz_target!(|data: &[u8]| {
    let _ = Update::deserialize(data);
    let _ = Update::deserialize_decrypted(data, descriptor());

    let mut prefixed = Vec::with_capacity(UPDATE_PREFIX.len() + data.len());
    prefixed.extend_from_slice(&UPDATE_PREFIX);
    prefixed.extend_from_slice(data);
    let _ = Update::deserialize(&prefixed);

    if let Ok(update) = Update::deserialize(data) {
        let encrypted = update
            .serialize_encrypted(descriptor())
            .expect("encryption succeeds");
        let decrypted =
            Update::deserialize_decrypted(&encrypted, descriptor()).expect("decryption succeeds");
        assert_eq!(update, decrypted);
    }
});
