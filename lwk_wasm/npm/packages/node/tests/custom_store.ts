import assert from "node:assert/strict";
import * as lwk from "@blockstream/lwk-node";

type StorageLike = {
  _data: Map<string, Uint8Array | null>;
  get(key: string): Uint8Array | null;
  put(key: string, value: Uint8Array | null): void;
  remove(key: string): void;
};

/**
 * A simple in-memory storage implementation in JavaScript.
 * This demonstrates the duck-typed storage pattern from the RFC.
 */
function createStorage(): StorageLike {
  const store = new Map<string, Uint8Array | null>();

  return {
    get(key: string) {
      return store.get(key) ?? null;
    },
    put(key: string, value: Uint8Array | null) {
      // IMPORTANT: Copy the value! WASM may reuse the underlying memory buffer.
      const valueCopy = value ? new Uint8Array(value) : null;
      store.set(key, valueCopy);
    },
    remove(key: string) {
      store.delete(key);
    },
    // Helper for testing - not required by the interface
    _data: store,
  };
}

function arrayEquals(
  left: ArrayBufferLike | ArrayLike<number> | null | undefined,
  right: ArrayBufferLike | ArrayLike<number> | null | undefined
): boolean {
  if (
    left === null ||
    left === undefined ||
    right === null ||
    right === undefined
  ) {
    return (
      (left === null || left === undefined) &&
      (right === null || right === undefined)
    );
  }

  const toUint8Array = (value: ArrayBufferLike | ArrayLike<number>) => {
    if (value instanceof Uint8Array) {
      return value;
    }

    if (
      value instanceof ArrayBuffer ||
      (typeof SharedArrayBuffer !== "undefined" &&
        value instanceof SharedArrayBuffer)
    ) {
      return new Uint8Array(value);
    }

    return Uint8Array.from(value as ArrayLike<number>);
  };

  const leftBytes = toUint8Array(left);
  const rightBytes = toUint8Array(right);

  if (leftBytes.length !== rightBytes.length) {
    return false;
  }

  for (let index = 0; index < leftBytes.length; index += 1) {
    if (leftBytes[index] !== rightBytes[index]) {
      return false;
    }
  }

  return true;
}

export async function runCustomStoreTest(): Promise<void> {
  const jsStorage = createStorage();
  const test = new lwk.JsTestStore(jsStorage);

  test.write("key", new Uint8Array([118, 97, 108, 117, 101])); // "value"
  const storedValue = jsStorage._data.get("key");
  assert.notEqual(
    storedValue,
    undefined,
    "Rust write should appear in JS storage"
  );

  let result = test.read("key");
  assert.notEqual(result, null, "Should read back the value");
  assert.ok(
    arrayEquals(result, new Uint8Array([118, 97, 108, 117, 101])),
    "Rust should read what was written"
  );

  result = test.read("nonexistent");
  assert.ok(
    result === null || result === undefined,
    "Non-existent key should return null/undefined"
  );

  test.write("key", new Uint8Array([110, 101, 119])); // "new"
  result = test.read("key");
  assert.ok(
    arrayEquals(result, new Uint8Array([110, 101, 119])),
    "Rust should read updated value"
  );

  test.remove("key");
  result = test.read("key");
  assert.ok(
    result === null || result === undefined,
    "Removed key should return null/undefined"
  );

  test.remove("key");

  test.write("Liquid:Tx:abc123", new Uint8Array([1, 2, 3]));
  test.write("Liquid:Addr:0", new Uint8Array([4, 5, 6]));

  result = test.read("Liquid:Tx:abc123");
  assert.ok(
    arrayEquals(result, new Uint8Array([1, 2, 3])),
    "Should read namespaced key"
  );

  result = test.read("Liquid:Addr:0");
  assert.ok(
    arrayEquals(result, new Uint8Array([4, 5, 6])),
    "Should read another namespaced key"
  );

  console.log("custom_store.ts: all tests passed");
}
