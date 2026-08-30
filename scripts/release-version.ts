const VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

function usage(): never {
  throw new Error(
    "Usage: release-version.ts <set|verify> <major.minor.patch>",
  );
}

const [operation, version] = Deno.args;
if ((operation !== "set" && operation !== "verify") || !version) usage();
if (!VERSION_PATTERN.test(version)) {
  throw new Error(
    `Release version must be stable SemVer without a leading v: ${version}`,
  );
}

const packagePath = new URL("../package.json", import.meta.url);
const tauriPath = new URL("../src-tauri/tauri.conf.json", import.meta.url);
const cargoPath = new URL("../src-tauri/Cargo.toml", import.meta.url);
const lockPath = new URL("../src-tauri/Cargo.lock", import.meta.url);

const packageJson = JSON.parse(await Deno.readTextFile(packagePath));
const tauriJson = JSON.parse(await Deno.readTextFile(tauriPath));
const cargoToml = await Deno.readTextFile(cargoPath);
const cargoLock = await Deno.readTextFile(lockPath);

function packageVersion(source: string, packageName: string): string {
  const packageStart = source.indexOf(`name = "${packageName}"`);
  if (packageStart < 0) {
    throw new Error(`Could not find ${packageName} package`);
  }
  const versionStart = source.indexOf('version = "', packageStart);
  const nextPackage = source.indexOf("\n[[package]]", packageStart + 1);
  if (versionStart < 0 || (nextPackage >= 0 && versionStart > nextPackage)) {
    throw new Error(`Could not find ${packageName} package version`);
  }
  const valueStart = versionStart + 'version = "'.length;
  const valueEnd = source.indexOf('"', valueStart);
  const found = source.slice(valueStart, valueEnd);
  if (!found) throw new Error(`Could not find ${packageName} package version`);
  return found;
}

const versions = {
  "package.json": String(packageJson.version),
  "src-tauri/tauri.conf.json": String(tauriJson.version),
  "src-tauri/Cargo.toml": packageVersion(cargoToml, "veilfeed"),
  "src-tauri/Cargo.lock": packageVersion(cargoLock, "veilfeed"),
};

if (operation === "verify") {
  const mismatches = Object.entries(versions).filter(([, value]) =>
    value !== version
  );
  if (mismatches.length) {
    throw new Error(
      `Expected ${version}; found ${
        mismatches.map(([file, value]) => `${file}=${value}`).join(", ")
      }`,
    );
  }
  console.log(`All release versions match ${version}.`);
  Deno.exit(0);
}

function replacePackageVersion(source: string, packageName: string): string {
  const packageStart = source.indexOf(`name = "${packageName}"`);
  if (packageStart < 0) {
    throw new Error(`Could not find ${packageName} package`);
  }
  const versionStart = source.indexOf('version = "', packageStart);
  const nextPackage = source.indexOf("\n[[package]]", packageStart + 1);
  if (versionStart < 0 || (nextPackage >= 0 && versionStart > nextPackage)) {
    throw new Error(`Could not find ${packageName} version`);
  }
  const valueStart = versionStart + 'version = "'.length;
  const valueEnd = source.indexOf('"', valueStart);
  return source.slice(0, valueStart) + version + source.slice(valueEnd);
}

packageJson.version = version;
tauriJson.version = version;
await Deno.writeTextFile(
  packagePath,
  `${JSON.stringify(packageJson, null, 2)}\n`,
);
await Deno.writeTextFile(tauriPath, `${JSON.stringify(tauriJson, null, 2)}\n`);
await Deno.writeTextFile(
  cargoPath,
  replacePackageVersion(cargoToml, "veilfeed"),
);
await Deno.writeTextFile(
  lockPath,
  replacePackageVersion(cargoLock, "veilfeed"),
);
console.log(`Set Veilfeed release version to ${version}.`);
