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

import { crateRoot } from "./paths.js";
import { run } from "./commands.js";

function appendRustCfg(existing: string | undefined): string {
  const flag = "--cfg=web_sys_unstable_apis";

  if (!existing) {
    return flag;
  }

  return existing.includes(flag) ? existing : `${existing} ${flag}`;
}

export function runWasmPack(
  target: "bundler" | "nodejs",
  outDir: string
): void {
  const args = [
    "build",
    crateRoot,
    "--target",
    target,
    "--out-dir",
    outDir,
    "--",
    "--features",
    "serial,simplicity",
  ];

  run("wasm-pack", args, {
    CARGO_PROFILE_RELEASE_OPT_LEVEL:
      process.env.CARGO_PROFILE_RELEASE_OPT_LEVEL ?? "z",
    RUSTFLAGS: appendRustCfg(process.env.RUSTFLAGS),
  });
}
