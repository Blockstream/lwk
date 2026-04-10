import assert from "node:assert/strict";
import test from "node:test";

import {
  WalletAbiAmountFilter,
  WalletAbiAssetFilter,
  WalletAbiLockFilter,
  WalletAbiWalletSourceFilter,
} from "lwk_wallet_abi_sdk/schema";
import { createWalletInput, generateRequestId } from "lwk_wallet_abi_sdk/builders";
import { assetIdFromString } from "lwk_wallet_abi_sdk/helpers";

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
