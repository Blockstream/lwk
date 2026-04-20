import { describe, expect, test } from "vitest";

import { formatJson, parseRawEnvelopeJson } from "../src/request.js";
import {
  GREEN_ANDROID_DEMO_SPLIT_ENVELOPE,
  GREEN_ANDROID_ISSUANCE_ENVELOPE,
  GREEN_ANDROID_REISSUANCE_ENVELOPE,
} from "./fixtures/android.js";

describe("android raw parity fixtures", () => {
  test("round-trips the Green split demo envelope", () => {
    const parsed = parseRawEnvelopeJson(
      formatJson(GREEN_ANDROID_DEMO_SPLIT_ENVELOPE),
    );

    expect(parsed).toEqual(GREEN_ANDROID_DEMO_SPLIT_ENVELOPE);
  });

  test("round-trips the Green issuance envelope", () => {
    const parsed = parseRawEnvelopeJson(
      formatJson(GREEN_ANDROID_ISSUANCE_ENVELOPE),
    );

    expect(parsed).toEqual(GREEN_ANDROID_ISSUANCE_ENVELOPE);
  });

  test("round-trips the Green reissuance envelope", () => {
    const parsed = parseRawEnvelopeJson(
      formatJson(GREEN_ANDROID_REISSUANCE_ENVELOPE),
    );

    expect(parsed).toEqual(GREEN_ANDROID_REISSUANCE_ENVELOPE);
  });
});
