import assert from "node:assert/strict";
import test from "node:test";

import { generateRequestId } from "lwk_wallet_abi_sdk/builders";

test("builder generates request ids", () => {
  const requestId = generateRequestId();
  assert.match(
    requestId,
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u
  );
});
