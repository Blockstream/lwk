/**
 * Small subprocess wrapper for the packaging scripts.
 *
 * The workspace build shells out to `wasm-pack`. This helper keeps the
 * subprocess behavior consistent: always run from the workspace root, inherit
 * stdio, and merge environment overrides in one place.
 */

import { execFileSync } from "node:child_process";

import { packageRoot } from "./paths.js";

export function run(
  command: string,
  args: string[],
  env?: NodeJS.ProcessEnv,
): void {
  execFileSync(command, args, {
    cwd: packageRoot,
    env: {
      ...process.env,
      ...env,
    },
    stdio: "inherit",
  });
}
