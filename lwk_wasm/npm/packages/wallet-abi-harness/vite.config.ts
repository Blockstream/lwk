import { defineConfig } from "vitest/config";
import wasmPlugin from "vite-plugin-wasm";

const wasm = wasmPlugin as unknown as () => unknown;

export default defineConfig({
  plugins: [wasm() as never],
  build: {
    target: "esnext",
  },
  server: {
    host: "0.0.0.0",
    port: 4178,
  },
  preview: {
    host: "0.0.0.0",
    port: 4178,
  },
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
  },
});
