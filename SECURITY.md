# Security Policy

Thanks for helping keep Tabularis and its users safe. We take security reports
seriously and appreciate responsible disclosure.

## Supported versions

Only the **latest released version** receives security fixes. Fixes are shipped
in a new release rather than backported to older ones, so please upgrade to the
most recent version before reporting an issue.

## Reporting a vulnerability

**Please do not open a public issue for security problems**, and don't disclose
the details publicly until a fix has shipped.

Use one of these private channels instead:

1. **GitHub private vulnerability reporting** (preferred) — go to the
   [**Security** tab](https://github.com/TabularisDB/tabularis/security/advisories)
   of this repository and click **Report a vulnerability**. This keeps the
   report private and lets us collaborate on the advisory and fix in one place.
2. **Email** — if you'd rather not use GitHub, write to
   **andrea@debbaweb.it** with `[SECURITY]` in the subject.

### What to include

The more of this you can provide, the faster we can confirm and fix:

- A clear description of the issue and its impact.
- The affected component, file, and (if known) the line or function.
- Steps to reproduce, or a proof of concept.
- The threat model you're assuming (e.g. untrusted MCP input, a malicious
  connected server, a prompt-injected agent turn, a local operator).
- Any suggested fix or mitigation.

## Scope

Tabularis is a desktop database client with an MCP layer that lets AI agents run
queries against your databases. Reports that are especially in scope include:

- Bypasses of the MCP safety guarantees — read-only mode or the write-approval
  gate (for example, a statement that executes writes/DDL but is classified as a
  read).
- SQL that reaches a database in a way the safety layer was expected to block.
- Exposure of stored credentials or connection secrets.
- Any way untrusted input (a connected MCP server, a prompt-injected agent) can
  perform actions the user did not approve.

Please assume the realistic threat model where the query text reaching Tabularis
may come from an untrusted source — that's exactly what the safety layer exists
to contain.

## Local profile and release isolation

Official release binaries contain no environment-specific connection targets or
credentials. On Windows, the application keeps its private profile under
`%APPDATA%\tabularis` and adopts that profile independently of the executable's
file name or download location.

- Database and SSH connection metadata is stored as an AES-256-GCM envelope.
  Its random key is held by the current user's OS credential store and is never
  compiled into the executable.
- Database, SSH, connection-URI, WebDAV, and audit credentials are stored in the
  OS credential store. Connection IDs remain stable so upgrades can retrieve
  existing credentials without prompting again.
- The first compatible launch migrates a legacy plaintext profile only after an
  encrypted recovery copy is durable. Atomic rollback copies protect interrupted
  writes; migration never replaces the only readable copy.
- Application logs use bounded rolling files. Credential material and connection
  metadata are redacted before a record reaches memory, disk, or support export.
- Local recovery material remains private and complete. Events sent to a remote
  audit endpoint use stable pseudonyms and SQL with literals removed.

The encrypted profile is intentionally bound to the same OS user. Before moving
to another computer, changing Windows accounts, or downgrading to a version that
predates encrypted profiles, export the profile from the current version and
store that export securely.

## Our commitment

- We'll acknowledge your report within **3 business days**.
- We'll keep you updated on our progress toward a fix.
- We'll credit you in the release notes and the advisory once the fix ships,
  unless you prefer to stay anonymous.

Thank you for reporting responsibly.
