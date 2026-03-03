const assert = require('assert');

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(`${message}: expected "${expected}", got "${actual}"`);
  }
}

function assertBytesEqual(a, b, message) {
  assert.deepStrictEqual(Array.from(a), Array.from(b), message);
}

function assertNotNull(value, message) {
  if (value === null || value === undefined) {
    throw new Error(`${message}: value is null or undefined`);
  }
}

function hasStatic(type, name) {
  return type && typeof type[name] === 'function';
}

function assertStringAndBytesRoundtrip(Type, inputString) {
  const typeName = getTypeName(Type);

  if (!hasStatic(Type, 'fromString') || !hasStatic(Type, 'fromBytes')) {
    return;
  }
  const value = Type.fromString(inputString);

  const canonical = value.toString();
  const bytes = value.toBytes();

  const fromBytes = Type.fromBytes(bytes);
  assert.strictEqual(fromBytes.toString(), canonical, `${typeName} fromBytes->toString roundtrip mismatch`);
  assertBytesEqual(fromBytes.toBytes(), bytes, `${typeName} toBytes roundtrip mismatch`);
  const fromCanonical = Type.fromString(canonical);
  assert.strictEqual(fromCanonical.toString(), canonical, `${typeName} canonical fromString roundtrip mismatch`);
}

function getTypeName(Type) {
  if (typeof Type?.typeName === "string" && Type.typeName) return Type.typeName;

  if (typeof Type === "function" && Type.name) return Type.name;

  if (Type?.constructor?.name && Type.constructor.name !== "Object") {
    return Type.constructor.name;
  }

  if (Type?.prototype?.constructor?.name) {
    return Type.prototype.constructor.name;
  }

  return "<unknown type>";
}

module.exports = {
  assertEqual,
  assertBytesEqual,
  assertNotNull,
  hasStatic,
  assertStringAndBytesRoundtrip,
};