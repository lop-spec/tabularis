import { readFileSync, writeFileSync } from 'fs';
import { resolve } from 'path';

// File paths
const paths = {
  package: resolve('package.json'),
  tauri: resolve('src-tauri/tauri.conf.json'),
  cargo: resolve('src-tauri/Cargo.toml'),
  appVersion: resolve('src/version.ts'),
  website: resolve('website/index.html'),
  readme: resolve('README.md')
};

// 1. Read the new version from package.json (already updated by npm version)
const pkg = JSON.parse(readFileSync(paths.package, 'utf-8'));
const newVersion = pkg.version;

console.log(`🔄 Syncing version to ${newVersion}...`);

// 2. Update tauri.conf.json
const tauriConf = JSON.parse(readFileSync(paths.tauri, 'utf-8'));
tauriConf.version = newVersion;
// Also update the version in the package node if present
if (tauriConf.package) tauriConf.package.version = newVersion;
writeFileSync(paths.tauri, JSON.stringify(tauriConf, null, 2));
console.log('✅ Updated tauri.conf.json');

// 3. Update Cargo.toml
let cargo = readFileSync(paths.cargo, 'utf-8');
// Use a regex to replace only the version in the [package] block
cargo = cargo.replace(
  /^version = ".*"/m,
  `version = "${newVersion}"`
);
writeFileSync(paths.cargo, cargo);
console.log('✅ Updated Cargo.toml');

// 4. Update src/version.ts
const versionContent = `export const APP_VERSION = "${newVersion}";\n`;
writeFileSync(paths.appVersion, versionContent);
console.log('✅ Updated src/version.ts');

// 5. Update website/index.html
let website = readFileSync(paths.website, 'utf-8');

// Update version badge: <span class="badge version">v0.6.0</span>
website = website.replace(
  /<span class="badge version">v.*?<\/span>/,
  `<span class="badge version">v${newVersion}</span>`
);

// Update download links (both tag vX.Y.Z in URL and filename tabularis_X.Y.Z_)
website = website.replace(
  /releases\/download\/v.*?\//g,
  `releases/download/v${newVersion}/`
);

website = website.replace(
  /tabularis_\d+\.\d+\.\d+_/g,
  `tabularis_${newVersion}_`
);

writeFileSync(paths.website, website);
console.log('✅ Updated website/index.html');

let readme = readFileSync(paths.readme, 'utf-8');

// Update download links in README
readme = readme.replace(
  /releases\/download\/v.*?\//g,
  `releases/download/v${newVersion}/`
);

readme = readme.replace(
  /tabularis_\d+\.\d+\.\d+_/g,
  `tabularis_${newVersion}_`
);

writeFileSync(paths.readme, readme);
console.log('✅ Updated README.md');
