import { mkdir, writeFile } from "node:fs/promises";

import { SignClient } from "@walletconnect/sign-client";

import {
  createGetRawSigningXOnlyPubkeyRequest,
  createGetSignerReceiveAddressRequest,
  parseGetRawSigningXOnlyPubkeyResponse,
  parseGetSignerReceiveAddressResponse,
  parseProcessRequestResponse,
} from "lwk_wallet_abi_sdk";
import { createProcessRequest } from "lwk_wallet_abi_sdk/protocol";
import {
  awaitWalletAbiApprovedSession,
  createWalletAbiMetadata,
  createWalletAbiRequiredNamespaces,
  createWalletConnectRequester,
  walletAbiNetworkToWalletConnectChain,
  type WalletAbiWalletConnectChain,
} from "lwk_wallet_abi_sdk/walletconnect";

import { scenarioToTxCreateRequest } from "../src/request.js";
import { createDefaultScenario } from "../src/scenario.js";

type LiveNetwork = "liquid" | "testnet-liquid" | "localtest-liquid";

interface LiveSummary {
  chainId: WalletAbiWalletConnectChain;
  topic: string;
  receiveAddress: string;
  signingPubkey: string;
  transfer: unknown;
  split: unknown;
}

const INLINE_ICON =
  "data:image/svg+xml;base64," +
  Buffer.from(
    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><rect width=\"64\" height=\"64\" rx=\"16\" fill=\"#0f172a\"/><path d=\"M20 22h24v20H20z\" fill=\"#22c55e\"/><path d=\"M26 30h12v4H26z\" fill=\"#0f172a\"/></svg>",
    "utf8",
  ).toString("base64");

function requiredEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (value) {
    return value;
  }

  throw new Error(`Expected ${name}`);
}

function readNetwork(): LiveNetwork {
  const candidate = process.env.WALLET_ABI_NETWORK?.trim();
  switch (candidate) {
    case undefined:
    case "":
      return "testnet-liquid";
    case "liquid":
    case "testnet-liquid":
    case "localtest-liquid":
      return candidate;
    default:
      throw new Error(`Unsupported WALLET_ABI_NETWORK: ${candidate}`);
  }
}

function readTimeout(name: string, fallbackMs: number): number {
  const raw = process.env[name]?.trim();
  if (!raw) {
    return fallbackMs;
  }

  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`Expected positive integer for ${name}`);
  }
  return parsed;
}

function delay(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

function describeError(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return String(error);
}

function prettyJson(value: unknown): string {
  return JSON.stringify(
    value,
    (_key, candidate) =>
      typeof candidate === "bigint" ? candidate.toString() : candidate,
    2,
  );
}

async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  message: string,
): Promise<T> {
  return await Promise.race([
    promise,
    new Promise<T>((_resolve, reject) => {
      setTimeout(() => {
        reject(new Error(message));
      }, timeoutMs);
    }),
  ]);
}

async function maybeWriteFile(path: string | undefined, contents: string) {
  if (!path) {
    return;
  }

  const normalized = path.trim();
  if (!normalized) {
    return;
  }

  const directory = normalized.replace(/\/[^/]+$/u, "");
  if (directory && directory !== normalized) {
    await mkdir(directory, { recursive: true });
  }
  await writeFile(normalized, contents, "utf8");
}

async function main() {
  const projectId =
    process.env.REOWN_PROJECT_ID?.trim() ||
    process.env.VITE_WALLETCONNECT_PROJECT_ID?.trim() ||
    requiredEnv("REOWN_PROJECT_ID");
  const network = readNetwork();
  const chainId = walletAbiNetworkToWalletConnectChain(network);
  const connectTimeoutMs = readTimeout(
    "WALLET_ABI_CONNECT_TIMEOUT_MS",
    180_000,
  );
  const requestTimeoutMs = readTimeout(
    "WALLET_ABI_REQUEST_TIMEOUT_MS",
    180_000,
  );
  const appUrl =
    process.env.WALLET_ABI_APP_URL?.trim() || "http://127.0.0.1:8787/";
  const pairingUriPath = process.env.WALLET_ABI_PAIRING_URI_PATH?.trim();
  const resultPath = process.env.WALLET_ABI_RESULT_PATH?.trim();

  const metadata = createWalletAbiMetadata(appUrl, {
    name: "Wallet ABI Harness Live E2E",
    description:
      "Host-side WalletConnect requester for Green Android transfer/split live testing.",
    icons: [INLINE_ICON],
  });

  console.log(`[live-e2e] initializing SignClient for ${chainId}`);
  const signClient = await SignClient.init({
    projectId,
    metadata,
    customStoragePrefix: `wallet-abi-live-${Date.now()}`,
  });

  let topic: string | null = null;

  try {
    const { uri, approval } = await signClient.connect({
      requiredNamespaces: createWalletAbiRequiredNamespaces(chainId),
    });

    if (!uri) {
      throw new Error("WalletConnect connect() did not return a pairing URI");
    }

    await maybeWriteFile(pairingUriPath, `${uri}\n`);
    console.log(`[live-e2e] pairing URI ready`);
    console.log(uri);

    const session = await awaitWalletAbiApprovedSession({
      approval,
      signClient,
      chainId,
      connectTimeoutMs,
    });
    topic = session.topic;
    console.log(`[live-e2e] session approved on topic ${topic}`);

    const requester = createWalletConnectRequester({
      chainId,
      getTopic: () => topic,
      client: {
        request({ topic: currentTopic, chainId: currentChainId, request }) {
          if (!currentTopic) {
            throw new Error("WalletConnect topic is not available");
          }

          return signClient.request({
            topic: currentTopic,
            chainId: currentChainId,
            request,
          });
        },
      },
    });

    let rpcId = 0;
    const nextRpcId = () => {
      rpcId += 1;
      return rpcId;
    };

    const getSignerReceiveAddressRequest =
      createGetSignerReceiveAddressRequest(nextRpcId());
    console.log(
      `[live-e2e] -> ${getSignerReceiveAddressRequest.method}\n${prettyJson(getSignerReceiveAddressRequest)}`,
    );
    const signerReceiveAddressResponse = await withTimeout(
      requester.request(getSignerReceiveAddressRequest),
      requestTimeoutMs,
      "Timed out waiting for get_signer_receive_address",
    );
    console.log(
      `[live-e2e] <- ${getSignerReceiveAddressRequest.method}\n${prettyJson(signerReceiveAddressResponse)}`,
    );
    const receiveAddress = parseGetSignerReceiveAddressResponse(
      signerReceiveAddressResponse,
    );

    const getRawSigningXOnlyPubkeyRequest =
      createGetRawSigningXOnlyPubkeyRequest(nextRpcId());
    console.log(
      `[live-e2e] -> ${getRawSigningXOnlyPubkeyRequest.method}\n${prettyJson(getRawSigningXOnlyPubkeyRequest)}`,
    );
    const rawSigningXOnlyPubkeyResponse = await withTimeout(
      requester.request(getRawSigningXOnlyPubkeyRequest),
      requestTimeoutMs,
      "Timed out waiting for get_raw_signing_x_only_pubkey",
    );
    console.log(
      `[live-e2e] <- ${getRawSigningXOnlyPubkeyRequest.method}\n${prettyJson(rawSigningXOnlyPubkeyResponse)}`,
    );
    const signingPubkey = parseGetRawSigningXOnlyPubkeyResponse(
      rawSigningXOnlyPubkeyResponse,
    );

    const transferScenario = createDefaultScenario("transfer");
    const transferRequest = await scenarioToTxCreateRequest({
      ...transferScenario,
      network,
      broadcast: true,
    });
    const transferEnvelope = createProcessRequest(nextRpcId(), transferRequest);
    console.log(
      `[live-e2e] -> transfer\n${prettyJson(transferEnvelope)}`,
    );
    const transferResponse = await withTimeout(
      requester.request(transferEnvelope),
      requestTimeoutMs,
      "Timed out waiting for transfer response",
    );
    console.log(`[live-e2e] <- transfer\n${prettyJson(transferResponse)}`);
    const transferResult = parseProcessRequestResponse(transferResponse).toJSON();

    await delay(1_500);

    const splitScenario = createDefaultScenario("split");
    const splitRequest = await scenarioToTxCreateRequest({
      ...splitScenario,
      network,
      broadcast: true,
    });
    const splitEnvelope = createProcessRequest(nextRpcId(), splitRequest);
    console.log(`[live-e2e] -> split\n${prettyJson(splitEnvelope)}`);
    const splitResponse = await withTimeout(
      requester.request(splitEnvelope),
      requestTimeoutMs,
      "Timed out waiting for split response",
    );
    console.log(`[live-e2e] <- split\n${prettyJson(splitResponse)}`);
    const splitResult = parseProcessRequestResponse(splitResponse).toJSON();

    const summary: LiveSummary = {
      chainId,
      topic,
      receiveAddress,
      signingPubkey,
      transfer: transferResult,
      split: splitResult,
    };

    console.log(`[live-e2e] summary\n${prettyJson(summary)}`);
    await maybeWriteFile(
      resultPath,
      `${prettyJson(summary)}\n`,
    );
  } finally {
    if (topic) {
      await signClient.disconnect({
        topic,
        reason: {
          code: 6_000,
          message: "Wallet ABI live E2E completed",
        },
      }).catch((error) => {
        console.warn(
          `[live-e2e] disconnect warning: ${describeError(error)}`,
        );
      });
    }

    const disconnectRelayer = (
      signClient.core.relayer as { disconnect?: () => Promise<void> }
    ).disconnect;
    if (disconnectRelayer !== undefined) {
      await disconnectRelayer.call(signClient.core.relayer).catch((error) => {
        console.warn(
          `[live-e2e] relayer disconnect warning: ${describeError(error)}`,
        );
      });
    }
  }
}

await main().catch(async (error) => {
  const message = describeError(error);
  console.error(`[live-e2e] failed: ${message}`);
  const resultPath = process.env.WALLET_ABI_RESULT_PATH?.trim();
  await maybeWriteFile(
    resultPath,
    `${prettyJson({
      error: message,
    })}\n`,
  ).catch(() => undefined);
  process.exitCode = 1;
});
