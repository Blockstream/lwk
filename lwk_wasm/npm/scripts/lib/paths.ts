/**
 * Shared filesystem layout for the npm workspace build.
 *
 * The workspace has two distinct outputs:
 * - `generated/`: raw `wasm-pack` output
 * - `packages/<name>/dist/generated`: staged runtime artifacts for each package
 */

import { resolve } from "node:path";

export type Mode = "clean" | "generate" | "stage-web" | "stage-node" | "stage";

export const packageRoot = resolve(".");
export const crateRoot = resolve("..");
export const packagesRoot = resolve("packages");
export const generatedRoot = resolve("generated");
export const webGeneratedRoot = resolve(generatedRoot, "web");
export const nodeGeneratedRoot = resolve(generatedRoot, "node");
export const webPackageRoot = resolve(packagesRoot, "web");
export const nodePackageRoot = resolve(packagesRoot, "node");
export const webDistRoot = resolve(webPackageRoot, "dist");
export const nodeDistRoot = resolve(nodePackageRoot, "dist");
export const webDistGeneratedRoot = resolve(webDistRoot, "generated");
export const nodeDistGeneratedRoot = resolve(nodeDistRoot, "generated");
export const generatedFeaturesPath = resolve(generatedRoot, ".features");
