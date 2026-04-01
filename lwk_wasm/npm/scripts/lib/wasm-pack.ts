/**
 * `wasm-pack` invocation helpers for browser and Node targets.
 *
 * We intentionally publish two runtime-specific packages from one Rust crate:
 * - `bundler` for `@blockstream/lwk-web`
 * - `nodejs` for `@blockstream/lwk-node`
 *
 * The wasm build also needs `--cfg=web_sys_unstable_apis` so the `serial`
 * feature keeps working in the browser build without depending on the caller's
 * shell environment.
 */

import { crateRoot, generatedFeaturesPath } from "./paths.js";
import { run } from "./commands.js";
import { existsSync, readFileSync } from "node:fs";

function appendRustCfg(existing: string | undefined): string {
  const flag = "--cfg=web_sys_unstable_apis";

  if (!existing) {
    return flag;
  }

  return existing.includes(flag) ? existing : `${existing} ${flag}`;
}

export function getWasmFeatures(): string {
  return (process.env.LWK_WASM_FEATURES ?? "serial")
    .split(",")
    .map((feature) => feature.trim())
    .filter(Boolean)
    .sort((left, right) => left.localeCompare(right))
    .join(",");
}

export function hasCurrentGeneratedFeatures(): boolean {
  if (!existsSync(generatedFeaturesPath)) {
    return false;
  }

  return (
    readFileSync(generatedFeaturesPath, "utf8").trim() === getWasmFeatures()
  );
}

export function runWasmPack(
  target: "bundler" | "nodejs",
  outDir: string
): void {
  const features = getWasmFeatures();
  const args = ["build", crateRoot, "--target", target, "--out-dir", outDir];

  if (features) {
    args.push("--", "--features", features);
  }

  const env: NodeJS.ProcessEnv = {
    CARGO_PROFILE_RELEASE_OPT_LEVEL:
      process.env.CARGO_PROFILE_RELEASE_OPT_LEVEL ?? "z",
  };

  if (features.split(",").includes("serial")) {
    env.RUSTFLAGS = appendRustCfg(process.env.RUSTFLAGS);
  }

  run("wasm-pack", args, env);
}
