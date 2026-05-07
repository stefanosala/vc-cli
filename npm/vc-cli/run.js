#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import semverGt from "semver/functions/gt.js";
import updateNotifier from "update-notifier";

const require = createRequire(import.meta.url);
const packageJson = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8")
);

// Write update notifications to stderr so command JSON on stdout stays clean.
const notifier = updateNotifier({ pkg: packageJson });
const update = notifier.update;
if (update && semverGt(update.latest, packageJson.version)) {
  notifier.config.set("update", update);
  const msg =
    `Update available: ${packageJson.version} -> ${update.latest}\n` +
    `Run "npm i -g ${packageJson.name}" to update`;
  process.stderr.write(`\n${msg}\n\n`);
}

const PLATFORM_PACKAGES = {
  "darwin-arm64": "@stefanosala/vc-cli-darwin-arm64",
  "linux-x64": "@stefanosala/vc-cli-linux-x64",
  "win32-x64": "@stefanosala/vc-cli-win32-x64",
};

const platformKey = `${process.platform}-${process.arch}`;
const platformPackage = PLATFORM_PACKAGES[platformKey];

if (!platformPackage) {
  console.error(
    `Unsupported platform: ${process.platform} ${process.arch}. ` +
      `Supported: ${Object.keys(PLATFORM_PACKAGES).join(", ")}`
  );
  process.exit(1);
}

const binaryName = process.platform === "win32" ? "vc-cli.exe" : "vc-cli";

let binaryPath;
try {
  binaryPath = join(
    dirname(require.resolve(`${platformPackage}/package.json`)),
    binaryName
  );
} catch {
  console.error(
    `Could not find package ${platformPackage}. This usually means the platform ` +
      `package was not installed. Try reinstalling: npm i -g @stefanosala/vc-cli`
  );
  process.exit(1);
}

try {
  execFileSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });
} catch (error) {
  if (error.signal) {
    process.kill(process.pid, error.signal);
  } else {
    process.exit(error.status ?? 1);
  }
}
