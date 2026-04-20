import type { WalletAbiMethod } from "lwk_wallet_abi_sdk/protocol";

import { isWalletAbiMethod } from "lwk_wallet_abi_sdk/protocol";

import { normalizeScenario, type ScenarioV1 } from "./scenario.js";

export interface HarnessRawEnvelope {
  id: number | string;
  jsonrpc: string;
  method: WalletAbiMethod;
  params?: unknown;
}

export interface DecodedHarnessLocation {
  scenario: ScenarioV1;
  rawEnvelope: HarnessRawEnvelope | null;
}

function toBase64Url(bytes: Uint8Array): string {
  if (typeof Buffer !== "undefined") {
    return Buffer.from(bytes)
      .toString("base64")
      .replace(/\+/gu, "-")
      .replace(/\//gu, "_")
      .replace(/=+$/u, "");
  }

  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }

  return btoa(binary)
    .replace(/\+/gu, "-")
    .replace(/\//gu, "_")
    .replace(/=+$/u, "");
}

function fromBase64Url(value: string): Uint8Array {
  const normalized = value.replace(/-/gu, "+").replace(/_/gu, "/");
  const padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4);

  if (typeof Buffer !== "undefined") {
    return new Uint8Array(Buffer.from(padded, "base64"));
  }

  const binary = atob(padded);
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseJsonFromHash<TValue>(value: string): TValue {
  const bytes = fromBase64Url(value);
  const text = new TextDecoder().decode(bytes);
  return JSON.parse(text) as TValue;
}

function encodeJsonForHash(value: unknown): string {
  const text = JSON.stringify(value);
  return toBase64Url(new TextEncoder().encode(text));
}

export function parseRawEnvelope(value: unknown): HarnessRawEnvelope | null {
  if (!isRecord(value)) {
    return null;
  }

  const { id, jsonrpc, method } = value;
  if (
    (typeof id !== "number" && typeof id !== "string") ||
    typeof jsonrpc !== "string"
  ) {
    return null;
  }

  if (typeof method !== "string" || !isWalletAbiMethod(method)) {
    return null;
  }

  return {
    id,
    jsonrpc,
    method,
    ...(Object.prototype.hasOwnProperty.call(value, "params")
      ? { params: value.params }
      : {}),
  };
}

export function encodeScenarioHash(scenario: ScenarioV1): string {
  return `scenario=${encodeJsonForHash(scenario)}`;
}

export function encodeRawEnvelopeHash(envelope: HarnessRawEnvelope): string {
  return `raw=${encodeJsonForHash(envelope)}`;
}

export function decodeHarnessLocation(hash: string): DecodedHarnessLocation {
  const trimmedHash = hash.startsWith("#") ? hash.slice(1) : hash;
  const searchParams = new URLSearchParams(trimmedHash);
  const rawParam = searchParams.get("raw");
  const scenarioParam = searchParams.get("scenario");

  let rawEnvelope: HarnessRawEnvelope | null = null;
  if (rawParam !== null) {
    try {
      rawEnvelope = parseRawEnvelope(parseJsonFromHash(rawParam));
    } catch {
      rawEnvelope = null;
    }
  }

  let scenario = normalizeScenario(undefined);
  if (scenarioParam !== null) {
    try {
      scenario = normalizeScenario(parseJsonFromHash(scenarioParam));
    } catch {
      scenario = normalizeScenario(undefined);
    }
  }

  return {
    scenario,
    rawEnvelope,
  };
}

export function createShareUrl(
  locationHref: string,
  input: {
    scenario: ScenarioV1;
    rawEnvelope?: HarnessRawEnvelope | null;
  },
): string {
  const url = new URL(locationHref);
  if (input.rawEnvelope !== undefined && input.rawEnvelope !== null) {
    url.hash = encodeRawEnvelopeHash(input.rawEnvelope);
    return url.toString();
  }

  url.hash = encodeScenarioHash(input.scenario);
  return url.toString();
}
