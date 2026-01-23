import { defineConfig } from 'vitest/config';
import { readFile } from 'fs/promises';
import { resolve } from 'path';

export default defineConfig({
  test: {
    include: ['tests/**/*.test.ts'],
    environment: 'node',
    globals: true,
    deps: {
      // Force vitest to process these through vite (allows plugins to work)
      inline: [/lwk_bindings\/pkg/],
    },
  },
  resolve: {
    alias: {
      // Mock the "env" module that wasm-bindgen generates imports for
      env: resolve(__dirname, 'tests/env-stub.ts'),
    },
  },
  plugins: [
    {
      name: 'wasm-loader',
      async load(id) {
        if (id.endsWith('.wasm')) {
          const buffer = await readFile(id);
          return `export default new Uint8Array([${buffer.join(',')}]);`;
        }
      },
    },
  ],
});
