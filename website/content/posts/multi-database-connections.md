---
title: "One Connection, Many Databases"
date: "2026-03-02"
release: "v0.9.3"
tags: ["mysql", "connections", "ux", "databases"]
excerpt: "v0.9.3 lets you select multiple databases in a single MySQL connection. Each appears as its own expandable node in the sidebar — no extra connections needed."
og:
  title: "One Connection,"
  accent: "Many Databases."
  claim: "Select multiple MySQL databases from a single connection and navigate them all from the sidebar."
  image: "/img/screenshot-9.png"
---

# One Connection, Many Databases

If you work with MySQL in a microservices environment, you already know the pain: ten services, ten databases, all on the same server — and until now, ten separate connections in Tabularis. **v0.9.3 changes that.**

## The Problem with One-Database-Per-Connection

MySQL is unusual among relational databases: a single connection can read and write across all databases on the server. You don't need separate TCP connections to `users_db`, `orders_db`, and `products_db`. Yet most database clients force you to configure one connection per database, cluttering your sidebar and multiplying your credentials to manage.

Tabularis v0.9.3 finally reflects how MySQL actually works.

## Selecting Multiple Databases

When creating or editing a MySQL (or compatible) connection, you'll find a new **Databases** tab in the connection form alongside the General and SSH tabs.

Click **Load Databases** to fetch all databases visible to your user. Then check the ones you want to include in this connection. A search box filters the list when you're dealing with dozens of databases. **Select All** and **Deselect All** shortcuts handle bulk operations.

The selected databases are saved with the connection. Next time you open it, Tabularis reconnects to the primary database and lazily loads the table lists for each selected database as you expand them.

## The Sidebar Experience

Each selected database appears as its own collapsible node in the Explorer sidebar — the same pattern Tabularis already uses for PostgreSQL schemas. Expand a node to see its tables and views. Double-click a table to open it in the editor.

Cross-database queries work exactly as you'd expect: Tabularis uses fully qualified names (`database_name.table_name`) when opening tables from non-primary databases, so MySQL resolves them correctly regardless of which database the connection was initially opened against.

## Backward Compatibility

Existing connections with a single database string continue to work without any changes. The connection format now accepts either a string (`"mydb"`) or an array (`["db1", "db2", "db3"]`), and both are handled transparently.

This feature is scoped to drivers that support cross-database access from a single connection — MySQL, MariaDB, and compatible engines. SQLite connections (file-based) and PostgreSQL connections (which use schemas instead) are unaffected.

## What's Next

The multi-database model opens the door to cross-database query autocompletion and ER diagrams that span multiple databases on the same server. Both are on the roadmap.

For now: configure once, navigate everything.

---

*v0.9.3 is available now. Update via the in-app updater or download from the releases page.*
