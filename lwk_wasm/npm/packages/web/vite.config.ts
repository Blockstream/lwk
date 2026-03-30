import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";
import topLevelAwait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

const packageRoot = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  build: {
    emptyOutDir: true,
    outDir: resolve(packageRoot, "../../.tmp/browser-smoke"),
    lib: {
      entry: resolve(packageRoot, "tests/browser-smoke.ts"),
      fileName: () => "browser-smoke.js",
      formats: ["es"],
    },
    rollupOptions: {
      treeshake: false,
    },
  },
  resolve: {
    conditions: ["module", "import", "default"],
  },
});
