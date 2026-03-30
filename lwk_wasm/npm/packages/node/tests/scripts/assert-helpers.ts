import assert from "node:assert/strict";

type RoundtripValue = {
  toBytes(): Uint8Array;
  toString(): string;
};

type RoundtripType = {
  fromBytes?(bytes: Uint8Array): RoundtripValue;
  fromString?(value: string): RoundtripValue;
  name?: string;
  typeName?: string;
};

export function assertEqual(
  actual: string | number | bigint,
  expected: string | number | bigint,
  message: string
): void {
  if (actual !== expected) {
    throw new Error(`${message}: expected "${expected}", got "${actual}"`);
  }
}

export function assertBytesEqual(
  left: Uint8Array,
  right: Uint8Array,
  message: string
): void {
  assert.deepEqual(Array.from(left), Array.from(right), message);
}

export function assertNotNull<T>(
  value: T | null | undefined,
  message: string
): asserts value is T {
  if (value === null || value === undefined) {
    throw new Error(`${message}: value is null or undefined`);
  }
}

function hasStatic(type: RoundtripType, name: keyof RoundtripType): boolean {
  return typeof type?.[name] === "function";
}

function getTypeName(type: RoundtripType): string {
  if (typeof type.typeName === "string" && type.typeName) {
    return type.typeName;
  }

  if (typeof type.name === "string" && type.name) {
    return type.name;
  }

  return "<unknown type>";
}

export function assertStringAndBytesRoundtrip(
  type: RoundtripType,
  inputString: string
): void {
  const typeName = getTypeName(type);

  if (!hasStatic(type, "fromString") || !hasStatic(type, "fromBytes")) {
    return;
  }

  const value = type.fromString!(inputString);
  const canonical = value.toString();
  const bytes = value.toBytes();

  const fromBytes = type.fromBytes!(bytes);
  assert.equal(
    fromBytes.toString(),
    canonical,
    `${typeName} fromBytes->toString roundtrip mismatch`
  );
  assertBytesEqual(
    fromBytes.toBytes(),
    bytes,
    `${typeName} toBytes roundtrip mismatch`
  );

  const fromCanonical = type.fromString!(canonical);
  assert.equal(
    fromCanonical.toString(),
    canonical,
    `${typeName} canonical fromString roundtrip mismatch`
  );
}
