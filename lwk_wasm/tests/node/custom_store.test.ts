import { expect } from "chai";
import * as lwk from "lwk_node";

/**
 * A simple in-memory storage implementation in JavaScript.
 * This demonstrates the duck-typed storage pattern from the RFC.
 */
interface JsStorage {
  get(key: string): Uint8Array | null;
  put(key: string, value: Uint8Array | null): void;
  remove(key: string): void;
  _data: Map<string, Uint8Array | null>;
}

function createStorage(): JsStorage {
  const store = new Map<string, Uint8Array | null>();
  return {
    get(key: string): Uint8Array | null {
      return store.get(key) || null;
    },
    put(key: string, value: Uint8Array | null): void {
      // IMPORTANT: Copy the value! WASM may reuse the underlying memory buffer.
      const valueCopy = value ? new Uint8Array(value) : null;
      store.set(key, valueCopy);
    },
    remove(key: string): void {
      store.delete(key);
    },
    // Helper for testing - not required by the interface
    _data: store,
  };
}

describe("CustomStore", function () {
  it("should perform storage operations via Rust/WASM", function () {
    // Create a JavaScript storage object
    const jsStorage = createStorage();

    // Create the test helper wrapping the JS storage
    const test = new lwk.JsTestStore(jsStorage);

    // Test Rust writing to JS storage
    test.write("key", new Uint8Array([118, 97, 108, 117, 101])); // "value"
    const storedValue = jsStorage._data.get("key");
    expect(storedValue, "Rust write should appear in JS storage").to.not.be
      .undefined;

    // Test Rust reading from JS storage
    let result = test.read("key");
    expect(result, "Should read back the value").to.not.be.null;
    expect(Array.from(result!)).to.deep.equal(
      Array.from(new Uint8Array([118, 97, 108, 117, 101]))
    );

    // Test Rust reading non-existent key
    result = test.read("nonexistent");
    // wasm-bindgen may return undefined instead of null for Option::None
    expect(
      result === null || result === undefined,
      "Non-existent key should return null/undefined"
    ).to.be.true;

    // Test Rust overwriting
    test.write("key", new Uint8Array([110, 101, 119])); // "new"
    result = test.read("key");
    expect(Array.from(result!)).to.deep.equal(
      Array.from(new Uint8Array([110, 101, 119]))
    );

    // Test Rust removing
    test.remove("key");
    result = test.read("key");
    expect(
      result === null || result === undefined,
      "Removed key should return null/undefined"
    ).to.be.true;

    // Test remove non-existent key (should not throw)
    test.remove("key");

    // Test with namespaced keys (as intended for LWK usage)
    test.write("Liquid:Tx:abc123", new Uint8Array([1, 2, 3]));
    test.write("Liquid:Addr:0", new Uint8Array([4, 5, 6]));

    result = test.read("Liquid:Tx:abc123");
    expect(Array.from(result!)).to.deep.equal(Array.from(new Uint8Array([1, 2, 3])));

    result = test.read("Liquid:Addr:0");
    expect(Array.from(result!)).to.deep.equal(Array.from(new Uint8Array([4, 5, 6])));
  });
});
