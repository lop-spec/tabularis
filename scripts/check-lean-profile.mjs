import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import { dirname, extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));

const forbiddenPaths = [
  ".claude/skills/tabularis-plugin-driver",
  "plugins",
  "packages/plugin-api",
  "packages/create-plugin",
  "src-tauri/src/ai.rs",
  "src-tauri/src/ai_activity.rs",
  "src-tauri/src/ai_approval.rs",
  "src-tauri/src/ai_commands.rs",
  "src-tauri/src/ai_notebook_export.rs",
  "src-tauri/src/ai_schema_context.rs",
  "src-tauri/src/heartbeat.rs",
  "src-tauri/src/mcp",
  "src-tauri/src/plugins",
  "src/pluginApi.ts",
  "src/pages/McpPage.tsx",
  "src/components/settings/AiTab.tsx",
  "src/components/settings/PluginsTab.tsx",
  "src/components/modals/AiQueryModal.tsx",
  "src/components/modals/McpModal.tsx",
  "src/components/modals/CommunityModal.tsx",
  "src/components/ui/SlotAnchor.tsx",
  "src/contexts/PluginSlotProvider.tsx",
];

const requiredPaths = [
  "src/config/shortcuts.json",
  "src/components/settings/ShortcutsTab.tsx",
  "src/contexts/KeybindingsContext.ts",
  "src/contexts/KeybindingsProvider.tsx",
  "src-tauri/src/native_cli.rs",
];

const forbiddenTokens = [
  "--mcp",
  "McpPage",
  "run_mcp_server",
  "get_mcp_status",
  "install_mcp_config",
  "install_plugin",
  "fetch_plugin_registry",
  "PluginSlotProvider",
  "SlotAnchor",
  "tauri-plugin-deep-link",
  "tauri_plugin_deep_link",
  "DiscordIcon",
  "DISCORD_URL",
  "discord.com/",
  "aiEnabled",
  "aiProvider",
  "aiModel",
  "set_ai_key",
  "generate_ai_query",
  "suggest_table_name",
  "generate_tab_rename",
  "generate_cell_name",
];

const scanTargets = [
  "src",
  "tests",
  "packages/explain/src",
  "src-tauri/src",
  "package.json",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
  "src-tauri/capabilities/default.json",
];

const ignoredDirectories = new Set([
  ".git",
  ".verification",
  "dist",
  "node_modules",
  "target",
]);

const ignoredFiles = new Set([
  "src/data/changelog.ts",
]);

const textExtensions = new Set([
  ".json",
  ".js",
  ".jsx",
  ".mjs",
  ".rs",
  ".toml",
  ".ts",
  ".tsx",
]);

const normalize = (path) => path.replaceAll("\\", "/");

function collectFiles(path) {
  const absolute = join(root, path);
  if (!existsSync(absolute)) return [];
  if (!statSync(absolute).isDirectory()) return [absolute];

  const files = [];
  for (const entry of readdirSync(absolute, { withFileTypes: true })) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue;
    const child = join(absolute, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectFiles(relative(root, child)));
    } else if (textExtensions.has(extname(entry.name))) {
      files.push(child);
    }
  }
  return files;
}

function hasAnyFile(path) {
  for (const entry of readdirSync(path, { withFileTypes: true })) {
    if (!entry.isDirectory()) return true;
    if (hasAnyFile(join(path, entry.name))) return true;
  }
  return false;
}

const failures = [];

for (const path of forbiddenPaths) {
  const absolute = join(root, path);
  const hasExcludedContent =
    existsSync(absolute) &&
    (!statSync(absolute).isDirectory() || hasAnyFile(absolute));
  if (hasExcludedContent) {
    failures.push(`excluded path exists: ${path}`);
  }
}

for (const path of requiredPaths) {
  if (!existsSync(join(root, path))) {
    failures.push(`required retained path is missing: ${path}`);
  }
}

const files = scanTargets.flatMap(collectFiles);
for (const file of files) {
  const localPath = normalize(relative(root, file));
  if (localPath.startsWith("src/i18n/") || ignoredFiles.has(localPath)) continue;
  const content = readFileSync(file, "utf8");
  for (const token of forbiddenTokens) {
    if (content.includes(token)) {
      failures.push(`excluded token ${JSON.stringify(token)} found in ${localPath}`);
    }
  }
}

const packageJson = readFileSync(join(root, "package.json"), "utf8");
if (!packageJson.includes('"check:lean-profile"')) {
  failures.push("package.json is missing the check:lean-profile script");
}

const rustEntry = readFileSync(join(root, "src-tauri/src/lib.rs"), "utf8");
for (const token of [
  "native_cli::register_manifests()",
  "commands::start_native_cli_session",
  "commands::close_native_cli_session",
]) {
  if (!rustEntry.includes(token)) {
    failures.push(`retained native CLI registration is missing: ${token}`);
  }
}

if (failures.length > 0) {
  console.error("Lean profile check failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Lean profile check passed (${files.length} active files scanned; shortcuts and native CLI retained).`,
);
