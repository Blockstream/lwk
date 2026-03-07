use crate::{Error, Network, SimplicityArguments, SimplicityWitnessValues, XOnlyPublicKey};

use lwk_simplicity::scripts;
use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;
use lwk_simplicity::wallet_abi::schema::{
    serialize_arguments, serialize_witness, RuntimeSimfValue, RuntimeSimfWitness, SimfArguments,
    SimfWitness,
};

use std::collections::HashMap;

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
struct WalletAbiTaprootHandleResult {
    handle: String,
    key: TaprootPubkeyGen,
}

#[wasm_bindgen(js_name = walletAbiSerializeArguments)]
pub fn wallet_abi_serialize_arguments(
    resolved: &SimplicityArguments,
    runtime_arguments: JsValue,
) -> Result<Vec<u8>, Error> {
    let runtime_arguments: HashMap<String, RuntimeSimfValue> =
        serde_wasm_bindgen::from_value(runtime_arguments)?;
    serialize_arguments_payload(resolved, runtime_arguments)
}

#[wasm_bindgen(js_name = walletAbiSerializeWitness)]
pub fn wallet_abi_serialize_witness(
    resolved: &SimplicityWitnessValues,
    runtime_arguments: JsValue,
) -> Result<Vec<u8>, Error> {
    let runtime_arguments: Vec<RuntimeSimfWitness> =
        serde_wasm_bindgen::from_value(runtime_arguments)?;
    serialize_witness_payload(resolved, runtime_arguments)
}

#[wasm_bindgen(js_name = walletAbiCreateTaprootHandle)]
pub fn wallet_abi_create_taproot_handle(
    source_simf: &str,
    resolved_arguments: &SimplicityArguments,
    network: &Network,
) -> Result<JsValue, Error> {
    Ok(serde_wasm_bindgen::to_value(
        &create_taproot_handle_payload(source_simf, resolved_arguments, network)?,
    )?)
}

#[wasm_bindgen(js_name = walletAbiCreateExternalTaprootHandle)]
pub fn wallet_abi_create_external_taproot_handle(
    source_simf: &str,
    resolved_arguments: &SimplicityArguments,
    x_only_public_key: &XOnlyPublicKey,
    network: &Network,
) -> Result<JsValue, Error> {
    Ok(serde_wasm_bindgen::to_value(
        &create_external_taproot_handle_payload(
            source_simf,
            resolved_arguments,
            x_only_public_key,
            network,
        )?,
    )?)
}

#[wasm_bindgen(js_name = walletAbiVerifyTaprootHandle)]
pub fn wallet_abi_verify_taproot_handle(
    handle: &str,
    source_simf: &str,
    resolved_arguments: &SimplicityArguments,
    network: &Network,
) -> Result<JsValue, Error> {
    Ok(serde_wasm_bindgen::to_value(
        &verify_taproot_handle_payload(handle, source_simf, resolved_arguments, network)?,
    )?)
}

fn serialize_arguments_payload(
    resolved: &SimplicityArguments,
    runtime_arguments: HashMap<String, RuntimeSimfValue>,
) -> Result<Vec<u8>, Error> {
    Ok(serialize_arguments(&SimfArguments {
        resolved: resolved.to_inner()?,
        runtime_arguments,
    })?)
}

fn serialize_witness_payload(
    resolved: &SimplicityWitnessValues,
    runtime_arguments: Vec<RuntimeSimfWitness>,
) -> Result<Vec<u8>, Error> {
    Ok(serialize_witness(&SimfWitness {
        resolved: resolved.to_inner()?,
        runtime_arguments,
    })?)
}

fn create_taproot_handle_payload(
    source_simf: &str,
    resolved_arguments: &SimplicityArguments,
    network: &Network,
) -> Result<WalletAbiTaprootHandleResult, Error> {
    let resolved_arguments = resolved_arguments.to_inner()?;
    let network = lwk_common::Network::from(network);
    let program = scripts::load_program(source_simf, resolved_arguments)?;
    let handle = TaprootPubkeyGen::from(&(), network, &|xonly, _, network| {
        Ok(scripts::create_p2tr_address(
            program.commit().cmr(),
            xonly,
            network.address_params(),
        ))
    })?;

    Ok(WalletAbiTaprootHandleResult {
        handle: handle.to_string(),
        key: handle,
    })
}

fn create_external_taproot_handle_payload(
    source_simf: &str,
    resolved_arguments: &SimplicityArguments,
    x_only_public_key: &XOnlyPublicKey,
    network: &Network,
) -> Result<WalletAbiTaprootHandleResult, Error> {
    let resolved_arguments = resolved_arguments.to_inner()?;
    let network = lwk_common::Network::from(network);
    let program = scripts::load_program(source_simf, resolved_arguments)?;
    let handle = TaprootPubkeyGen::from_external_x_only(
        (*x_only_public_key).into(),
        &(),
        network,
        &|xonly, _, network| {
            Ok(scripts::create_p2tr_address(
                program.commit().cmr(),
                xonly,
                network.address_params(),
            ))
        },
    )?;

    Ok(WalletAbiTaprootHandleResult {
        handle: handle.to_string(),
        key: handle,
    })
}

fn verify_taproot_handle_payload(
    handle: &str,
    source_simf: &str,
    resolved_arguments: &SimplicityArguments,
    network: &Network,
) -> Result<WalletAbiTaprootHandleResult, Error> {
    let resolved_arguments = resolved_arguments.to_inner()?;
    let network = lwk_common::Network::from(network);
    let program = scripts::load_program(source_simf, resolved_arguments)?;
    let handle = TaprootPubkeyGen::build_from_str(handle, &(), network, &|xonly, _, network| {
        Ok(scripts::create_p2tr_address(
            program.commit().cmr(),
            xonly,
            network.address_params(),
        ))
    })?;

    Ok(WalletAbiTaprootHandleResult {
        handle: handle.to_string(),
        key: handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimplicityTypedValue, XOnlyPublicKey};
    use lwk_wollet::hashes::hex::FromHex;

    #[test]
    fn serialize_arguments_payload_includes_runtime_arguments() {
        let resolved =
            SimplicityArguments::new().add_value("PUBLIC_KEY", &SimplicityTypedValue::from_u32(7));
        let bytes = serialize_arguments_payload(
            &resolved,
            HashMap::from([(
                "ISSUED_ASSET".to_string(),
                RuntimeSimfValue::NewIssuanceAsset { input_index: 0 },
            )]),
        )
        .expect("serialize arguments");

        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(
            value,
            serde_json::json!({
                "resolved": {
                    "PUBLIC_KEY": {
                        "type": "u32",
                        "value": "7"
                    }
                },
                "runtime_arguments": {
                    "ISSUED_ASSET": {
                        "new_issuance_asset": {
                            "input_index": 0
                        }
                    }
                }
            }),
        );
    }

    #[test]
    fn serialize_witness_payload_includes_runtime_directives() {
        let resolved = SimplicityWitnessValues::new().add_value(
            "STATIC_SIG",
            &SimplicityTypedValue::from_byte_array_hex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("byte array"),
        );
        let bytes = serialize_witness_payload(
            &resolved,
            vec![RuntimeSimfWitness::SigHashAll {
                name: "SIG_ALL".to_string(),
                public_key: lwk_wollet::elements::bitcoin::XOnlyPublicKey::from_slice(
                    &<[u8; 32]>::from_hex(
                        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
                    )
                    .expect("hex"),
                )
                .expect("xonly"),
            }],
        )
        .expect("serialize witness");

        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        assert_eq!(
            value,
            serde_json::json!({
                "resolved": {
                    "STATIC_SIG": {
                        "type": "[u8; 32]",
                        "value": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    }
                },
                "runtime_arguments": [
                    {
                        "sig_hash_all": {
                            "name": "SIG_ALL",
                            "public_key": "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                        }
                    }
                ]
            }),
        );
    }

    #[test]
    fn create_and_verify_taproot_handles() {
        let source_simf = include_str!("../../lwk_simplicity/data/p2pk.simf");
        let resolved = SimplicityArguments::new().add_value(
            "PUBLIC_KEY",
            &SimplicityTypedValue::from_u256_hex(
                "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083",
            )
            .expect("typed value"),
        );

        let random_handle =
            create_taproot_handle_payload(source_simf, &resolved, &Network::testnet())
                .expect("random handle");
        let verified_random = verify_taproot_handle_payload(
            &random_handle.handle,
            source_simf,
            &resolved,
            &Network::testnet(),
        )
        .expect("verify random");
        assert_eq!(verified_random.handle, random_handle.handle);

        let xonly = XOnlyPublicKey::from_string(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .expect("xonly");
        let external_handle = create_external_taproot_handle_payload(
            source_simf,
            &resolved,
            &xonly,
            &Network::testnet(),
        )
        .expect("external handle");
        assert!(external_handle.handle.starts_with("ext-"));

        let verified_external = verify_taproot_handle_payload(
            &external_handle.handle,
            source_simf,
            &resolved,
            &Network::testnet(),
        )
        .expect("verify external");
        assert_eq!(verified_external.handle, external_handle.handle);
    }
}
