import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { basename, resolve } from "node:path";
import { pathToFileURL } from "node:url";

type Runnable = () => Promise<void> | void;

function collectExports(
  module: Record<string, unknown>
): Record<string, unknown> {
  if (module.default && typeof module.default === "object") {
    return { ...module.default, ...module };
  }

  return module;
}

function findRunnable(
  module: Record<string, unknown>,
  fileName: string
): Runnable {
  const runnable = Object.values(collectExports(module)).find(
    (value): value is Runnable =>
      typeof value === "function" && value.name.startsWith("run")
  );

  if (!runnable) {
    throw new Error(`No run* export found in ${fileName}`);
  }

  return runnable;
}

async function runTestFile(testFile: string): Promise<void> {
  const fileUrl = pathToFileURL(testFile).href;
  const module = (await import(fileUrl)) as Record<string, unknown>;
  const runnable = findRunnable(module, basename(testFile));

  await runnable();
}

async function main(): Promise<void> {
  const singleTestFile = process.argv[2];

  if (singleTestFile) {
    await runTestFile(resolve(singleTestFile));
    return;
  }

  const testDir = resolve(__dirname, "..");
  const runnerPath = resolve(__dirname, "run-tests.ts");
  const tsxCli = resolve(
    __dirname,
    "../../../../node_modules/tsx/dist/cli.mjs"
  );
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
