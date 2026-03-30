/**
 * Version sync guard between Cargo and published npm package metadata.
 *
 * The browser and Node packages are both published from the Rust `lwk_wasm`
 * crate, so every workspace package version must match `lwk_wasm/Cargo.toml`.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { crateRoot, nodePackageRoot, webPackageRoot } from "./paths.js";

const publishedPackages = [
  {
    name: "@blockstream/lwk-web",
    root: webPackageRoot,
  },
  {
    name: "@blockstream/lwk-node",
    root: nodePackageRoot,
  },
] as const;

export function readCargoVersion(): string {
  const cargoToml = readFileSync(resolve(crateRoot, "Cargo.toml"), "utf8");
  const match = cargoToml.match(/^\s*version\s*=\s*"([^"]+)"/m);

  if (!match) {
    throw new Error("Could not find lwk_wasm package version in Cargo.toml");
  }

  return match[1];
}

export function readPublishedPackageVersions(): Array<{
  name: string;
  version: string;
}> {
  return publishedPackages.map(({ name, root }) => {
    const packageJson = JSON.parse(
      readFileSync(resolve(root, "package.json"), "utf8")
    ) as {
      version?: string;
    };

    if (!packageJson.version) {
      throw new Error(`Could not find package version in ${name}`);
    }

    return {
      name,
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
