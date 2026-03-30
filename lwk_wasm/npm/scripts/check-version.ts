import {
  ensureVersionsMatch,
  readPublishedPackageVersions,
} from "./lib/version.js";

const packageVersions = readPublishedPackageVersions();

ensureVersionsMatch();

console.log(
  `Version check passed: ${packageVersions
    .map(({ name, version }) => `${name}@${version}`)
    .join(", ")}`
);
