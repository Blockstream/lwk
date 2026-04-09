import json

from lwk import *

network = Network.testnet()
request_id = "0d6d53cd-a040-4f0c-8d28-c67b6608fb14"
xonly_hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
taproot_handle_string = (
    f"ext-{xonly_hex}:"
    "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:"
    "lq1qqvxk052kf3qtkxmrakx50a9gc3smqad2ync54hzntjt980kfej9kkfe0247rp5h4yzmdftsahhw64uy8pzfe7cpg4fgykm7cv"
)

utxo_source = WalletAbiUtxoSource.wallet(WalletAbiWalletSourceFilter.any())
unblinding = WalletAbiInputUnblinding.wallet()
finalizer = WalletAbiFinalizerSpec.wallet()
input_schema = WalletAbiInputSchema.new_with_sequence_consensus(
    "input-1",
    utxo_source,
    unblinding,
    0xFFFFFFFD,
    finalizer,
)

output_schema = WalletAbiOutputSchema(
    "output-1",
    1500,
    WalletAbiLockVariant.wallet(),
    WalletAbiAssetVariant.asset_id(network.policy_asset()),
    WalletAbiBlinderVariant.wallet(),
)

params = WalletAbiRuntimeParams([input_schema], [output_schema], 100.0, None)
request = WalletAbiTxCreateRequest.from_parts(
    request_id,
    Network.testnet(),
    params,
    False,
)
request_json = request.to_json()
request_roundtrip = WalletAbiTxCreateRequest.from_json(request_json)

assert request_roundtrip.to_json() == request_json
assert request_roundtrip.request_id() == request_id
assert request_roundtrip.abi_version() == "wallet-abi-0.1"

error = WalletAbiErrorInfo.from_code_string(
    "invalid_request",
    "bad request",
    "{\"field\":\"params\"}",
)
response = WalletAbiTxCreateResponse.error(
    request_id,
    Network.testnet(),
    error,
)
response_json = response.to_json()
response_roundtrip = WalletAbiTxCreateResponse.from_json(response_json)

assert response_roundtrip.to_json() == response_json
assert response_roundtrip.status() == WalletAbiStatus.ERROR
assert response_roundtrip.error_info().code() == WalletAbiErrorCode.INVALID_REQUEST
assert response_roundtrip.error_info().details_json() == "{\"field\":\"params\"}"

capabilities = WalletAbiCapabilities(
    Network.testnet(),
    [
        "wallet_abi_process_request",
        "get_signer_receive_address",
        "wallet_abi_evaluate_request",
    ],
)
capabilities_roundtrip = WalletAbiCapabilities.from_json(capabilities.to_json())

assert capabilities_roundtrip.abi_version() == "wallet-abi-0.1"
assert capabilities_roundtrip.network() == Network.testnet()
assert capabilities_roundtrip.methods() == [
    "get_signer_receive_address",
    "wallet_abi_evaluate_request",
    "wallet_abi_process_request",
]

preview_delta = WalletAbiPreviewAssetDelta(network.policy_asset(), -1500)
preview_output = WalletAbiPreviewOutput(
    WalletAbiPreviewOutputKind.FEE,
    network.policy_asset(),
    600,
    Script.empty(),
)
preview = WalletAbiRequestPreview(
    [preview_delta],
    [preview_output],
    ["requires review"],
)
preview_roundtrip = WalletAbiRequestPreview.from_json(preview.to_json())

assert preview_roundtrip.asset_deltas()[0].wallet_delta_sat() == -1500
assert preview_roundtrip.outputs()[0].kind() == WalletAbiPreviewOutputKind.FEE
assert preview_roundtrip.warnings() == ["requires review"]

evaluate_request = WalletAbiTxEvaluateRequest.from_parts(
    request_id,
    Network.testnet(),
    params,
)
evaluate_request_roundtrip = WalletAbiTxEvaluateRequest.from_json(
    evaluate_request.to_json()
)
evaluate_response = WalletAbiTxEvaluateResponse.ok(evaluate_request, preview)
evaluate_response_roundtrip = WalletAbiTxEvaluateResponse.from_json(
    evaluate_response.to_json()
)

assert evaluate_request_roundtrip.to_json() == evaluate_request.to_json()
assert evaluate_response_roundtrip.status() == WalletAbiStatus.OK
assert evaluate_response_roundtrip.preview().warnings() == ["requires review"]
assert evaluate_response_roundtrip.error_info() is None

transaction = WalletAbiTransactionInfo(
    "00",
    Txid("0000000000000000000000000000000000000000000000000000000000000000"),
)
ok_response = WalletAbiTxCreateResponse.ok(
    request_id,
    Network.testnet(),
    transaction,
    json.dumps({"preview": json.loads(preview.to_json())}),
)

assert ok_response.preview() is not None
assert ok_response.preview().outputs()[0].kind() == WalletAbiPreviewOutputKind.FEE
assert ok_response.preview().warnings() == ["requires review"]

taproot_handle = WalletAbiTaprootHandle.from_string(taproot_handle_string)

assert str(taproot_handle) == taproot_handle_string

simf_arguments = WalletAbiSimfArguments.from_resolved(SimplicityArguments())
simf_arguments = simf_arguments.append_runtime_argument(
    "issuance_asset",
    WalletAbiRuntimeSimfValue.new_issuance_asset(3),
)
simf_arguments_bytes = simf_arguments.to_bytes()
simf_arguments_roundtrip = WalletAbiSimfArguments.from_bytes(simf_arguments_bytes)
simf_runtime_argument = simf_arguments_roundtrip.runtime_argument("issuance_asset")

assert simf_arguments_roundtrip.to_bytes() == simf_arguments_bytes
assert simf_arguments_roundtrip.runtime_argument_names() == ["issuance_asset"]
assert simf_runtime_argument is not None
assert simf_runtime_argument.kind() == "new_issuance_asset"
assert simf_runtime_argument.input_index() == 3

xonly_public_key = XOnlyPublicKey.from_string(xonly_hex)
simf_witness = WalletAbiSimfWitness.from_resolved(SimplicityWitnessValues())
simf_witness = simf_witness.append_runtime_argument(
    WalletAbiRuntimeSimfWitness.sig_hash_all("sig_all", xonly_public_key),
)
simf_witness_bytes = simf_witness.to_bytes()
simf_witness_roundtrip = WalletAbiSimfWitness.from_bytes(simf_witness_bytes)
simf_runtime_witnesses = simf_witness_roundtrip.runtime_arguments()

assert simf_witness_roundtrip.to_bytes() == simf_witness_bytes
assert len(simf_runtime_witnesses) == 1
assert simf_runtime_witnesses[0].kind() == "sig_hash_all"
assert simf_runtime_witnesses[0].name() == "sig_all"
assert str(simf_runtime_witnesses[0].public_key()) == xonly_hex
