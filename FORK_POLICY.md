# Lean Fork Policy

This fork intentionally keeps the database client small and locally auditable.

## Retained surfaces

- Keyboard shortcuts and their settings UI.
- Built-in database drivers.
- Tauri framework plugins required by retained application features.
- Existing rollback-protection behavior and recovery history.

## Permanently excluded surfaces

- MongoDB/Redis drivers and their native consoles (removed in 6854377f).
- Discord promotion and community UI.
- AI providers, prompts, model discovery, activity, approvals, and AI-assisted actions.
- The built-in MCP server and its installer, approval flow, CLI flag, and UI.
- The external Tabularium driver/plugin registry, installer, runtime loader, extension slots, SDK, and scaffolding packages.

Official upstream merges must not reintroduce excluded surfaces. Run
`pnpm check:lean-profile` after resolving every upstream merge and before a
release build.

The application no longer reads or executes external plugin files. Existing
user plugin data is deliberately left in place so removal is non-destructive;
it may be archived or deleted separately by the user.

The gate must reject removed native-console registrations as well as external
plugins. MySQL, PostgreSQL and SQLite remain built in.
