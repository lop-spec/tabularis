import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const source = resolve(root, process.env.TABULARIS_PORTABLE_SOURCE || "src-tauri/target/release/tabularis.exe");
const outputDirectory = resolve(root, process.env.TABULARIS_PORTABLE_OUTPUT || "dist/portable");
const version = String(process.env.TABULARIS_PORTABLE_VERSION || packageJson.version)
  .replace(/^v/i, "")
  .trim();

if (!/^[0-9A-Za-z][0-9A-Za-z._-]*$/.test(version)) {
  throw new Error(`Invalid portable version: ${version}`);
}

const data = readFileSync(source);
if (data[0] !== 0x4d || data[1] !== 0x5a) {
  throw new Error("Portable artifact is not a Windows PE executable");
}
if (statSync(source).size < 5 * 1024 * 1024) {
  throw new Error("Portable artifact is unexpectedly small");
}

mkdirSync(outputDirectory, { recursive: true });
const fileName = `tabularis_${version}_x64-portable.exe`;
const output = join(outputDirectory, fileName);
copyFileSync(source, output);

const sha256 = createHash("sha256").update(data).digest("hex");
writeFileSync(join(outputDirectory, "SHA256SUMS.txt"), `${sha256}  ${fileName}\n`);

console.log(JSON.stringify({ output, bytes: data.length, sha256 }, null, 2));
