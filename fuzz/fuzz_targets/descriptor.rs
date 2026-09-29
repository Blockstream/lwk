#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use lwk_wollet::{Chain, Network, WolletDescriptor};

const MAX_INPUT_BYTES: usize = 4096;
const BLINDING_KEY: &str = "ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92";
const XPUB: &str = "tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA";
const DESCRIPTOR: &str = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))";

fn check_descriptor(descriptor: WolletDescriptor) {
    let canonical = descriptor.to_string();
    let reparsed = WolletDescriptor::from_str(&canonical).expect("displayed descriptor reparses");
    assert_eq!(descriptor, reparsed);

    let _ = descriptor.is_mainnet();
    let _ = descriptor.is_elip151();
    let _ = descriptor.descriptor();
    let _ = descriptor.ct_descriptor();
    let _ = descriptor.url_encoded_descriptor();
    let _ = descriptor.bitcoin_descriptor_without_key_origin();
    let _ = descriptor.encryption_key_bytes();

    for chain in [Chain::External, Chain::Internal] {
        for index in [0, 1] {
            let _ = descriptor.script_pubkey(chain, index);
            let _ = descriptor.definite_descriptor(chain, index);
            match chain {
                Chain::External => {
                    let _ = descriptor.address(index, Network::TestnetLiquid.address_params());
                }
                Chain::Internal => {
                    let _ = descriptor.change(index, Network::TestnetLiquid.address_params());
                }
            }
        }
    }
}

fn parse_strict(input: &str) {
    if let Ok(descriptor) = WolletDescriptor::from_str(input) {
        check_descriptor(descriptor);
    }
}

fn parse_relaxed(input: &str) {
    if let Ok(descriptor) = WolletDescriptor::from_str_relaxed(input) {
        check_descriptor(descriptor);
    }
}

fn mutate_template(template: &str, data: &[u8]) -> String {
    if data.is_empty() {
        return template.to_string();
    }

    let offset = usize::from(data[0]) % (template.len() + 1);
    let remove = data
        .get(1)
        .map(|byte| usize::from(*byte) % 9)
        .unwrap_or_default()
        .min(template.len() - offset);
    let mutation = String::from_utf8_lossy(&data[data.len().min(2)..data.len().min(66)]);

    let mut result = template.to_string();
    result.replace_range(offset..offset + remove, &mutation);
    result
}

fn exercise(data: &[u8]) {
    let input = String::from_utf8_lossy(data);

    parse_strict(&input);
    parse_relaxed(&input);
    parse_strict(&format!("ct({input})"));
    parse_strict(&format!("ct(slip77({input}),elwpkh({XPUB}))"));
    parse_strict(&format!("ct(slip77({BLINDING_KEY}),{input})"));
    parse_strict(&format!("ct(slip77({BLINDING_KEY}),elwpkh({input}))"));
    parse_strict(&format!("{BLINDING_KEY}:{input}"));
    parse_strict(&format!(":{input}"));
    parse_strict(&format!("{input}:{input}"));
    parse_relaxed(&format!("{input}\n{input}"));

    parse_strict(&mutate_template(DESCRIPTOR, data));

    let external = DESCRIPTOR.replace("<0;1>", "0");
    let internal = DESCRIPTOR.replace("<0;1>", "1");
    let external = mutate_template(&external, data);
    let internal = mutate_template(&internal, data);
    parse_relaxed(&format!("{external}\n{internal}"));
}

fuzz_target!(|data: &[u8]| {
    exercise(&data[..data.len().min(MAX_INPUT_BYTES)]);
});
