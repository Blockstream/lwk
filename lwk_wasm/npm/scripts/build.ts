/**
 * Workspace build entrypoint.
 *
 * The npm workspace publishes runtime-specific packages from one Rust crate:
 * - `lwk_node`
 * - `lwk_web`
 *
 * The build pipeline has two phases:
 * - `generated/`: raw `wasm-pack` output for browser and Node targets
 * - `packages/<name>/dist/generated`: staged runtime artifacts consumed by the
 *   package-local TypeScript builds
 */

import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { ensureVersionsMatch } from "./lib/version.js";
import {
  generatedFeaturesPath,
  generatedRoot,
  Mode,
  nodeDistGeneratedRoot,
  nodeDistRoot,
  nodeGeneratedRoot,
  webGeneratedRoot,
} from "./lib/paths.js";
import {
  getWasmFeatures,
  hasCurrentGeneratedFeatures,
  runWasmPack,
} from "./lib/wasm-pack.js";

const mode = (process.argv[2] ?? "stage") as Mode;
const nodeGeneratedFiles = [
  "lwk_wasm.js",
  "lwk_wasm.d.ts",
  "lwk_wasm_bg.wasm",
  "lwk_wasm_bg.wasm.d.ts",
] as const;

function clean(): void {
  rmSync(generatedRoot, { force: true, recursive: true });
  rmSync(nodeDistRoot, { force: true, recursive: true });
  rmSync(resolve(".tmp"), { force: true, recursive: true });
}

function generate(): void {
  ensureVersionsMatch();

  rmSync(generatedRoot, { force: true, recursive: true });
  mkdirSync(generatedRoot, { recursive: true });

  runWasmPack("nodejs", nodeGeneratedRoot);
  runWasmPack("bundler", webGeneratedRoot);

  writeFileSync(generatedFeaturesPath, `${getWasmFeatures()}\n`);
}

function ensureGenerated(): void {
  if (
    !existsSync(nodeGeneratedRoot) ||
    !hasCurrentGeneratedFeatures()
  ) {
    generate();
  }
}

function copyGeneratedFiles(
  sourceRoot: string,
  destinationRoot: string,
  files: readonly string[]
): void {
  mkdirSync(destinationRoot, { recursive: true });

  for (const file of files) {
    cpSync(resolve(sourceRoot, file), resolve(destinationRoot, file));
  }
}

function stageNode(): void {
  ensureGenerated();

  rmSync(nodeDistRoot, { force: true, recursive: true });
  mkdirSync(nodeDistRoot, { recursive: true });
  copyGeneratedFiles(
    nodeGeneratedRoot,
    nodeDistGeneratedRoot,
    nodeGeneratedFiles
  );
}

function stage(): void {
  stageNode();
}

switch (mode) {
  case "clean":
    clean();
    break;
  case "generate":
    generate();
    break;
  case "stage-node":
    stageNode();
    break;
  case "stage":
    stage();
    break;
  default:
    throw new Error(`Unknown build mode: ${mode}`);
}
