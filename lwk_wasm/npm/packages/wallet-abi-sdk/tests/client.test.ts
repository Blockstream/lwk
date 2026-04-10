import assert from "node:assert/strict";
import test from "node:test";

import {
  WalletAbiClient,
  WalletAbiClientError,
} from "lwk_wallet_abi_sdk/client";

test("wallet abi client connect is idempotent", async () => {
  let connectCalls = 0;
  const client = new WalletAbiClient({
    requester: {
      async connect() {
        connectCalls += 1;
      },
      request() {
        throw new WalletAbiClientError("unused");
      },
    },
  });

  await Promise.all([client.connect(), client.connect()]);
  await client.connect();

  assert.equal(connectCalls, 1);
  assert.equal(client.requestTimeoutMs(), 120_000);
});

test("wallet abi client disconnect is idempotent", async () => {
  let disconnectCalls = 0;
  const client = new WalletAbiClient({
    requester: {
      async connect() {},
      async disconnect() {
        disconnectCalls += 1;
      },
      request() {
        throw new WalletAbiClientError("unused");
      },
    },
    requestTimeoutMs: 5_000,
  });

  await client.connect();
  await client.disconnect();
  await client.disconnect();

  assert.equal(disconnectCalls, 1);
  assert.equal(client.requestTimeoutMs(), 5_000);
});

test("wallet abi client gets signer receive address", async () => {
  let requestCalls = 0;
  const client = new WalletAbiClient({
    requester: {
      async connect() {},
      request(request) {
        requestCalls += 1;
        assert.equal(request.method, "get_signer_receive_address");
        return {
          id: request.id,
          jsonrpc: "2.0",
          result:
            "tlq1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqf6u0sd",
        };
      },
    },
  });

  const address = await client.getSignerReceiveAddress();

  assert.equal(
    address,
    "tlq1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqf6u0sd"
  );
  assert.equal(requestCalls, 1);
});

test("wallet abi client gets raw signing xonly pubkey", async () => {
  let requestCalls = 0;
  const client = new WalletAbiClient({
    requester: {
      async connect() {},
      request(request) {
        requestCalls += 1;
        assert.equal(request.method, "get_raw_signing_x_only_pubkey");
        return {
          id: request.id,
          jsonrpc: "2.0",
          result:
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        };
      },
    },
  });

  const xonly = await client.getRawSigningXOnlyPubkey();

  assert.equal(
    xonly,
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
  );
  assert.equal(requestCalls, 1);
});
