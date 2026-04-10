import assert from "node:assert/strict";
import test from "node:test";

import {
  Script,
  WalletAbiAmountFilter,
  WalletAbiAssetVariant,
  WalletAbiAssetFilter,
  WalletAbiBlinderVariant,
  WalletAbiLockFilter,
  WalletAbiLockVariant,
  WalletAbiOutputSchema,
  WalletAbiRuntimeParams,
  WalletAbiWalletSourceFilter,
} from "lwk_wallet_abi_sdk/schema";
import {
  createProvidedInput,
  createTxCreateRequest,
  createWalletInput,
  generateRequestId,
} from "lwk_wallet_abi_sdk/builders";
import {
  assetIdFromString,
  networkFromString,
  outPointFromString,
} from "lwk_wallet_abi_sdk/helpers";

test("builder generates request ids", () => {
  const requestId = generateRequestId();
  assert.match(
    requestId,
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u
  );
});

test("builder creates wallet inputs", () => {
  const filter = WalletAbiWalletSourceFilter.withFilters(
    WalletAbiAssetFilter.exact(
      assetIdFromString(
        "144c654344aa716e1f3faf6f9bf8d832021ff5d6387b920ca43aa837ed3521f0"
      )
    ),
    WalletAbiAmountFilter.exact(5_000n),
    WalletAbiLockFilter.none()
  );

  const input = createWalletInput({
    id: "wallet-input",
    filter,
  });

  assert.equal(input.id(), "wallet-input");
  assert.equal(input.utxoSource().kind(), "wallet");
  assert.equal(input.utxoSource().walletFilter()?.amount().amountSat(), 5_000n);
});

test("builder creates provided inputs", () => {
  const input = createProvidedInput({
    id: "provided-input",
    outpoint: outPointFromString(
      "0000000000000000000000000000000000000000000000000000000000000000:1"
    ),
  });

  assert.equal(input.id(), "provided-input");
  assert.equal(input.utxoSource().kind(), "provided");
  assert.equal(
    input.utxoSource().providedOutpoint()?.txid().toString(),
    "0000000000000000000000000000000000000000000000000000000000000000"
  );
  assert.equal(input.utxoSource().providedOutpoint()?.vout(), 1);
  assert.equal(input.unblinding().kind(), "explicit");
});

test("builder creates tx-create requests", () => {
  const network = networkFromString("liquid-testnet");
  const params = WalletAbiRuntimeParams.new(
    [
      createWalletInput({
        id: "wallet-input",
      }),
    ],
    [
      WalletAbiOutputSchema.new(
        "output-0",
        1_000n,
        WalletAbiLockVariant.script(Script.empty()),
        WalletAbiAssetVariant.assetId(network.policyAsset()),
        WalletAbiBlinderVariant.explicit()
      ),
    ],
    100.0,
    null
  );

  const request = createTxCreateRequest({
    network,
    params,
  });

  assert.match(
    request.requestId(),
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u
  );
  assert.equal(request.network().isTestnet(), true);
  assert.equal(request.broadcast(), false);
  assert.equal(request.params().inputs()[0]?.id(), "wallet-input");
});
