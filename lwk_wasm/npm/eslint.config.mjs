import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

const sharedGlobals = {
  ...globals.browser,
  ...globals.node,
};

export default [
  {
    ignores: [
      "generated/**",
      "packages/*/dist/**",
      "packages/node/tests/**",
    ],
  },
  {
    files: ["**/*.{js,mjs,cjs,ts}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: sharedGlobals,
    },
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.ts"],
    rules: {
      "no-undef": "off",
    },
  },
  {
    files: ["**/*.cts"],
    rules: {
      "@typescript-eslint/no-require-imports": "off",
    },
  },
];
