import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

type Runnable = () => Promise<void> | void;

function collectExports(module: Record<string, unknown>): Record<string, unknown> {
  if (module.default && typeof module.default === "object") {
    return { ...module.default, ...module };
  }

  return module;
}

function findRunnable(
  module: Record<string, unknown>,
): Runnable {
  const exports = collectExports(module);

  const defaultExport = exports.default;
  if (typeof defaultExport === "function") {
    return defaultExport as Runnable;
  }

  // No callable export found. Assume the module executed via top-level side effects
  // when it was imported, so return a no-op runnable.
  return () => {};
}

async function runTestFile(testFile: string): Promise<void> {
  const fileUrl = pathToFileURL(testFile).href;
  const module = (await import(fileUrl)) as Record<string, unknown>;
  const runnable = findRunnable(module);

  await runnable();
}

async function main(): Promise<void> {
  const singleTestFile = process.argv[2];
  const requireFromHere = createRequire(__filename);

  if (singleTestFile) {
    await runTestFile(resolve(singleTestFile));
    return;
  }

  const testDir = resolve(__dirname, "..");
  const runnerPath = resolve(__dirname, "run-tests.ts");
  const tsxCli = requireFromHere.resolve("tsx/cli");
  const testFiles = readdirSync(testDir)
    .filter((entry) => entry.endsWith(".ts"))
    .sort();

  let failed = false;

  for (const fileName of testFiles) {
    console.log(`RUN  ${fileName}`);

    const testFile = resolve(testDir, fileName);
    const result = spawnSync(process.execPath, [tsxCli, runnerPath, testFile], {
      stdio: "inherit",
    });

    if (result.status === 0) {
      console.log(`PASS ${fileName}`);
    } else {
      failed = true;
      console.error(`FAIL ${fileName}`);

      if (result.error) {
        console.error(result.error);
      }
    }
  }

  if (failed) {
    throw new Error("Some tests failed");
  }

  console.log("All tests passed");
}

void main().catch((error) => {
  console.error(error);
  process.exit(1);
});
