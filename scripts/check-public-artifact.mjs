import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { extname, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const artifactFlag = process.argv.indexOf("--artifact");
const artifact = artifactFlag >= 0 ? resolve(process.argv[artifactFlag + 1] ?? "") : null;
const textExtensions = new Set([
  ".cjs",
  ".js",
  ".json",
  ".jsx",
  ".md",
  ".mjs",
  ".rs",
  ".sh",
  ".toml",
  ".ts",
  ".tsx",
  ".yaml",
  ".yml",
]);
const skippedFiles = new Set(["pnpm-lock.yaml", "src-tauri/Cargo.lock"]);
const findings = [];

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function isAllowedIpv4(value) {
  const octets = value.split(".").map(Number);
  if (octets.length !== 4 || octets.some((part) => !Number.isInteger(part) || part > 255)) {
    return true;
  }
  const [a, b] = octets;
  return (
    a === 0 ||
    a === 10 ||
    a === 127 ||
    a >= 224 ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    (a === 192 && b === 0 && octets[2] === 2) ||
    (a === 198 && b === 51 && octets[2] === 100) ||
    (a === 203 && b === 0 && octets[2] === 113)
  );
}

function lineNumber(text, index) {
  return text.slice(0, index).split("\n").length;
}

function scanText(label, text, binaryMode = false) {
  const ipv4 = /(?<![\d.])(?:\d{1,3}\.){3}\d{1,3}(?![\d.])/g;
  for (const match of text.matchAll(ipv4)) {
    if (!isAllowedIpv4(match[0]) && (!binaryMode || deniedHashes.has(sha256(match[0])))) {
      findings.push(`${label}:${lineNumber(text, match.index)} public IPv4 address`);
    }
  }

  const rules = [
    [/[a-z0-9-]+\.mysql\.rds\.aliyuncs\.com/gi, "cloud database endpoint"],
    [/\brm-?[a-z0-9]{14,}\b/gi, "cloud database instance identifier"],
    [/functional[_-](?:test|smoke)[a-z0-9_-]*/gi, "release-only functional test hook"],
  ];
  for (const [pattern, description] of rules) {
    for (const match of text.matchAll(pattern)) {
      if (binaryMode && description === "cloud database instance identifier" && !deniedHashes.has(sha256(match[0]))) {
        continue;
      }
      findings.push(`${label}:${lineNumber(text, match.index)} ${description}`);
    }
  }

  const credentialUri = /\b(?:mysql|mariadb|postgres(?:ql)?|mongodb(?:\+srv)?|redis):\/\/([^\s/@:]+):([^\s/@]+)@([^\s/]+)/gi;
  for (const match of text.matchAll(credentialUri)) {
    if (match[0].includes("{") || match[0].includes("}")) {
      continue;
    }
    const host = match[3].split(":", 1)[0].toLowerCase();
    if (!["localhost", "127.0.0.1", "::1"].includes(host) && !host.endsWith(".invalid")) {
      findings.push(`${label}:${lineNumber(text, match.index)} credential-bearing database URI`);
    }
  }
}

const denyPath = resolve(root, "security/public-artifact-deny.sha256");
const deniedHashes = new Set(
  readFileSync(denyPath, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim().split(/\s+/, 1)[0])
    .filter((line) => /^[a-f0-9]{64}$/.test(line)),
);

const trackedFiles = execFileSync(
  "git",
  ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
  { cwd: root },
)
  .toString("utf8")
  .split("\0")
  .filter(Boolean);
for (const relative of trackedFiles) {
  const absolute = resolve(root, relative);
  if (
    skippedFiles.has(relative) ||
    !textExtensions.has(extname(relative)) ||
    !existsSync(absolute)
  ) {
    continue;
  }
  scanText(relative, readFileSync(absolute, "utf8"));
}

if (artifact) {
  if (!existsSync(artifact)) {
    throw new Error(`Artifact does not exist: ${artifact}`);
  }
  const bytes = readFileSync(artifact);
  const printable = Buffer.from(
    bytes.map((byte) => (byte >= 0x20 && byte <= 0x7e ? byte : 0x0a)),
  ).toString("ascii");
  scanText(artifact, printable, true);
}

if (findings.length > 0) {
  console.error("Public artifact security gate failed:");
  for (const finding of findings) {
    console.error(`- ${finding}`);
  }
  process.exit(1);
}

console.log(`Public artifact security gate passed (${trackedFiles.length} tracked files${artifact ? ", artifact scanned" : ""}).`);
