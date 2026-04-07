/**
 * Version sync guard between Cargo and published npm package metadata.
 *
 * Runtime-specific npm packages are published from the Rust `lwk_wasm` crate,
 * so every workspace package version must match `lwk_wasm/Cargo.toml`.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { crateRoot, packagesRoot } from "./paths.js";

function readCargoVersion(): string {
  const cargoToml = readFileSync(resolve(crateRoot, "Cargo.toml"), "utf8");
  const match = cargoToml.match(/^\s*version\s*=\s*"([^"]+)"/m);

  if (!match) {
    throw new Error("Could not find lwk_wasm package version in Cargo.toml");
  }

  return match[1];
}

function readPublishedPackageVersions(): Array<{
  name: string;
  version: string;
}> {
  return readdirSync(packagesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => resolve(packagesRoot, entry.name))
    .filter((root) => existsSync(resolve(root, "package.json")))
    .sort((left, right) => left.localeCompare(right))
    .map((root) => {
      const packageJson = JSON.parse(
        readFileSync(resolve(root, "package.json"), "utf8")
      ) as {
        name?: string;
        version?: string;
      };

      if (!packageJson.name || !packageJson.version) {
        throw new Error(`Could not find package name/version in ${root}`);
      }

      return {
        name: packageJson.name,
        version: packageJson.version,
      };
    });
}

export function ensureVersionsMatch(): void {
  const cargoVersion = readCargoVersion();

  for (const { name, version } of readPublishedPackageVersions()) {
    if (cargoVersion !== version) {
      throw new Error(
        `Version mismatch: lwk_wasm/Cargo.toml is ${cargoVersion}, ${name} is ${version}`
      );
    }
  }
}
