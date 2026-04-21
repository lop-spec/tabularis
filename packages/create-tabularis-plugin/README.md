# create-tabularis-plugin

Scaffold a new [Tabularis](https://github.com/TabularisDB/tabularis) database driver plugin in seconds.

```bash
npx create-tabularis-plugin my-driver
```

## What you get

A runnable Rust project with:

- **`manifest.json`** aligned with the Tabularis plugin schema.
- **33 JSON-RPC handlers pre-wired** — metadata methods return empty arrays (plugin loads cleanly), query/CRUD/DDL methods return `-32601` until you implement them.
- **`test_connection` placeholder** that returns success, so your driver appears in the connection picker immediately after `just dev-install`.
- **Working utilities**: `quote_identifier`, `paginate` — with unit tests — ready to use from your handlers.
- **Cross-platform GitHub Actions release workflow** for Linux x64/arm64, macOS x64/arm64, and Windows x64.
- **Local REPL (`just repl`)** for debugging without restarting Tabularis.
- **Optional UI extension subworkspace** (`--with-ui`) pre-configured with Vite IIFE + `@tabularis/plugin-api`.

## Usage

```bash
npx create-tabularis-plugin [options] <name>
```

### Options

| Flag | Values | Default | Purpose |
|------|--------|---------|---------|
| `--db-type` | `network` \| `file` \| `folder` \| `api` | `network` | Shapes the connection form and capabilities |
| `--quote` | `"` \| `` ` `` | `"` | SQL identifier quote character |
| `--with-ui` | boolean | off | Also scaffold a `ui/` subworkspace using `@tabularis/plugin-api` |
| `--no-git` | boolean | off | Skip `git init` |
| `--dir` | path | `./<name>` | Target directory |

### Examples

```bash
# Network driver (host/port/user/pass connection form)
npx create-tabularis-plugin my-pg-like

# File-based driver (SQLite, DuckDB shape)
npx create-tabularis-plugin duckdb-clone --db-type=file

# API-based plugin (no connection form; public REST-ish data source)
npx create-tabularis-plugin my-api --db-type=api

# With UI extension scaffold
npx create-tabularis-plugin mine --with-ui
```

## Next steps after scaffolding

```bash
cd my-driver
just dev-install              # builds and installs into ~/.local/share/tabularis/plugins/my-driver
# open Tabularis → your driver is in the connection picker
```

From there, fill in handlers in `src/handlers/metadata.rs`, then `query.rs`, then the rest. The generated `README.md` includes a feature-by-feature roadmap.

## Layout of the generated project

```
my-driver/
├── Cargo.toml
├── manifest.json
├── README.md
├── justfile            # just build / test / dev-install / repl / lint / fmt
├── rust-toolchain.toml
├── .github/workflows/release.yml
└── src/
    ├── main.rs         # stdio JSON-RPC loop
    ├── rpc.rs          # method dispatch
    ├── client.rs       # TODO: your DB client
    ├── error.rs
    ├── models.rs
    ├── handlers/{metadata,query,crud,ddl}.rs
    ├── utils/{identifiers,pagination}.rs
    └── bin/test_plugin.rs
```

With `--with-ui`:

```
my-driver/ui/
├── package.json
├── tsconfig.json
├── vite.config.ts
└── src/index.tsx     # defineSlot("data-grid.toolbar.actions", …)
```

## Requirements

- Node 18.17 or newer.
- Rust stable (for building the generated plugin).
- `just` (optional but recommended — the generated `justfile` wraps the common tasks).

## Related

- **[Plugin guide](https://github.com/TabularisDB/tabularis/blob/main/plugins/PLUGIN_GUIDE.md)** — authoritative reference for JSON-RPC methods, capabilities, slots.
- **[`@tabularis/plugin-api`](https://www.npmjs.com/package/@tabularis/plugin-api)** — TypeScript types + hooks for UI extensions.
- **[Tabularis repo](https://github.com/TabularisDB/tabularis)** — the host app.

## License

Apache-2.0
