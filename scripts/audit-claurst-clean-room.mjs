import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vendorClaurst = resolve(root, "vendor", "claurst");

function normalizePath(value) {
  return resolve(value).toLowerCase();
}

function isUnderVendorClaurst(value) {
  const normalized = normalizePath(value);
  const vendor = normalizePath(vendorClaurst);
  return normalized === vendor || normalized.startsWith(`${vendor}${sep}`);
}

function fail(message) {
  console.error(`claurst clean-room audit failed: ${message}`);
  process.exit(1);
}

function readCargoMetadata() {
  const raw = execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  return JSON.parse(raw);
}

function auditCargoMetadata(metadata) {
  const offenders = [];

  for (const member of metadata.workspace_members ?? []) {
    if (member.toLowerCase().includes("vendor/claurst")) {
      offenders.push(`workspace member ${member}`);
    }
  }

  for (const pkg of metadata.packages ?? []) {
    if (pkg.manifest_path && isUnderVendorClaurst(pkg.manifest_path)) {
      offenders.push(`package ${pkg.name} manifest ${pkg.manifest_path}`);
    }
    for (const dep of pkg.dependencies ?? []) {
      if (dep.path && isUnderVendorClaurst(dep.path)) {
        offenders.push(`dependency ${pkg.name} -> ${dep.name} path ${dep.path}`);
      }
    }
  }

  if (offenders.length > 0) {
    fail(`vendor/claurst entered Cargo metadata:\n${offenders.join("\n")}`);
  }
}

function auditRootManifests() {
  const manifestPaths = [
    "Cargo.toml",
    "src-tauri/Cargo.toml",
    "crates/panes-agent/Cargo.toml",
  ];
  const offenders = [];

  for (const manifestPath of manifestPaths) {
    const absolutePath = resolve(root, manifestPath);
    const raw = readFileSync(absolutePath, "utf8");
    if (/path\s*=\s*["'][^"']*vendor[\\/]+claurst/i.test(raw)) {
      offenders.push(manifestPath);
    }
  }

  if (offenders.length > 0) {
    fail(`manifest path dependency points at vendor/claurst: ${offenders.join(", ")}`);
  }
}

function auditVendorReadme() {
  const readmePath = resolve(root, "vendor", "README.md");
  if (!existsSync(readmePath)) {
    fail("vendor/README.md is missing");
  }
  const readme = readFileSync(readmePath, "utf8").toLowerCase();
  for (const required of ["vendor/claurst", "gpl-3.0", "reference", "not compiled"]) {
    if (!readme.includes(required)) {
      fail(`vendor/README.md does not mention required claurst status: ${required}`);
    }
  }
}

const metadata = readCargoMetadata();
auditCargoMetadata(metadata);
auditRootManifests();
auditVendorReadme();

const relativeVendor = relative(root, vendorClaurst) || "vendor/claurst";
console.log(`claurst clean-room audit passed; ${relativeVendor} is not in the Cargo build graph`);
