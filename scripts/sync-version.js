import { readFileSync, writeFileSync } from 'fs';
import { resolve } from 'path';

// Percorsi dei file
const paths = {
  package: resolve('package.json'),
  tauri: resolve('src-tauri/tauri.conf.json'),
  cargo: resolve('src-tauri/Cargo.toml'),
  appVersion: resolve('src/version.ts'),
  website: resolve('website/index.html'),
  readme: resolve('README.md')
};

// 1. Leggi la nuova versione da package.json (che è già stato aggiornato da npm version)
const pkg = JSON.parse(readFileSync(paths.package, 'utf-8'));
const newVersion = pkg.version;

console.log(`🔄 Syncing version to ${newVersion}...`);

// 2. Aggiorna tauri.conf.json
const tauriConf = JSON.parse(readFileSync(paths.tauri, 'utf-8'));
tauriConf.version = newVersion;
// Aggiorna anche la versione nel nodo package se presente (Tauri v2 non lo usa sempre, ma per sicurezza)
if (tauriConf.package) tauriConf.package.version = newVersion;
writeFileSync(paths.tauri, JSON.stringify(tauriConf, null, 2));
console.log('✅ Updated tauri.conf.json');

// 3. Aggiorna Cargo.toml
let cargo = readFileSync(paths.cargo, 'utf-8');
// Usa una regex per sostituire solo la versione nel blocco [package]
// Cerca "version =" seguito da virgolette, ma solo nelle prime righe per evitare di cambiare le dipendenze
cargo = cargo.replace(
  /^version = ".*"/m,
  `version = "${newVersion}"`
);
writeFileSync(paths.cargo, cargo);
console.log('✅ Updated Cargo.toml');

// 4. Aggiorna src/version.ts
const versionContent = `export const APP_VERSION = "${newVersion}";\n`;
writeFileSync(paths.appVersion, versionContent);
console.log('✅ Updated src/version.ts');

// 5. Aggiorna website/index.html
let website = readFileSync(paths.website, 'utf-8');

// Aggiorna il badge della versione: <span class="badge version">v0.6.0</span>
website = website.replace(
  /<span class="badge version">v.*?<\/span>/,
  `<span class="badge version">v${newVersion}</span>`
);

// Aggiorna i link di download (sia il tag vX.Y.Z nell'URL che il nome file tabularis_X.Y.Z_)
// URL: releases/download/v0.6.0/tabularis_0.6.0_...
// Sostituisce tutte le occorrenze globalmente
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

// 6. Aggiorna README.md download badges
let readme = readFileSync(paths.readme, 'utf-8');

// Aggiorna i link di download nel README (sia il tag vX.Y.Z nell'URL che il nome file tabularis_X.Y.Z_)
// URL: releases/download/v0.6.0/tabularis_0.6.0_...
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
