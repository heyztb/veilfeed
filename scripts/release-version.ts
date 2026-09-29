const [operation, version] = Deno.args;
if (
  !["set", "verify"].includes(operation) ||
  !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version ?? "")
) {
  throw new Error("Usage: release-version.ts <set|verify> <major.minor.patch>");
}
const root = new URL("../", import.meta.url);
const workspacePath = new URL("Cargo.toml", root);
const packagePath = new URL("package.json", root);
const tauriPath = new URL("src-tauri/tauri.conf.json", root);
const packageJson = JSON.parse(await Deno.readTextFile(packagePath));
const tauriJson = JSON.parse(await Deno.readTextFile(tauriPath));
if (operation === "set") {
  const source = await Deno.readTextFile(workspacePath);
  await Deno.writeTextFile(
    workspacePath,
    source.replace(
      /(\[workspace\.package\]\s*\nversion = ")[^"]+("\s*)/,
      `$1${version}$2`,
    ),
  );
  packageJson.version = version;
  tauriJson.version = version;
  await Deno.writeTextFile(
    packagePath,
    JSON.stringify(packageJson, null, 2) + "\n",
  );
  await Deno.writeTextFile(
    tauriPath,
    JSON.stringify(tauriJson, null, 2) + "\n",
  );
  const update = await new Deno.Command("cargo", {
    args: ["update", "--workspace", "--offline"],
    cwd: root,
  }).output();
  if (!update.success) throw new Error(new TextDecoder().decode(update.stderr));
}
const result = await new Deno.Command("cargo", {
  args: [
    "metadata",
    "--no-deps",
    "--locked",
    "--offline",
    "--format-version",
    "1",
  ],
  cwd: root,
}).output();
if (!result.success) throw new Error(new TextDecoder().decode(result.stderr));
const metadata = JSON.parse(new TextDecoder().decode(result.stdout));
for (const name of ["veilfeed", "veilfeed-core"]) {
  const pkg = metadata.packages.find((pkg: { name: string }) =>
    pkg.name === name
  );
  if (pkg?.version !== version) {
    throw new Error(`${name}: expected ${version}, found ${pkg?.version}`);
  }
}
const lockfile = await Deno.readTextFile(new URL("Cargo.lock", root));
for (const name of ["veilfeed", "veilfeed-core"]) {
  const locked = lockfile.split("[[package]]").map((entry) =>
    entry.match(/^name = "([^"]+)"\nversion = "([^"]+)"/m)
  ).find((match) => match?.[1] === name)?.[2];
  if (locked !== version) {
    throw new Error(`Cargo.lock ${name}: expected ${version}, found ${locked}`);
  }
}
if (packageJson.version !== version || tauriJson.version !== version) {
  throw new Error("Tauri metadata must match the workspace version");
}
console.log(`All release versions match ${version}.`);
