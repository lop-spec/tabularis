export interface RelatedLink {
  href: string;
  label: string;
}

const TAG_TO_LINKS: Record<string, RelatedLink[]> = {
  notebooks: [
    { href: "/solutions/sql-notebooks", label: "SQL notebooks guide" },
    { href: "/wiki/notebooks", label: "Notebook documentation" },
  ],
  mcp: [
    { href: "/solutions/mcp-database-client", label: "MCP database client guide" },
    { href: "/wiki/mcp-server", label: "MCP setup documentation" },
  ],
  ai: [
    { href: "/solutions/mcp-database-client", label: "MCP database client guide" },
    { href: "/wiki/ai-assistant", label: "AI assistant documentation" },
  ],
  sql: [
    { href: "/solutions/postgresql-client", label: "PostgreSQL client guide" },
    { href: "/wiki/editor", label: "SQL editor documentation" },
  ],
  postgres: [
    { href: "/solutions/postgresql-client", label: "PostgreSQL client guide" },
    { href: "/compare/datagrip-alternative", label: "DataGrip alternative" },
  ],
  plugins: [
    { href: "/plugins", label: "Plugin registry" },
    { href: "/wiki/plugins", label: "Plugin documentation" },
  ],
  plugin: [
    { href: "/plugins", label: "Plugin registry" },
    { href: "/wiki/plugins", label: "Plugin documentation" },
  ],
  visual: [
    { href: "/wiki/visual-query-builder", label: "Visual query builder docs" },
  ],
};

const WIKI_TO_LINKS: Record<string, RelatedLink[]> = {
  editor: [
    { href: "/solutions/postgresql-client", label: "PostgreSQL client guide" },
    { href: "/solutions/sqlite-client-for-developers", label: "SQLite client guide" },
  ],
  notebooks: [
    { href: "/solutions/sql-notebooks", label: "SQL notebooks guide" },
    { href: "/compare/dbeaver-alternative", label: "DBeaver alternative" },
  ],
  "mcp-server": [
    { href: "/solutions/mcp-database-client", label: "MCP database client guide" },
    { href: "/compare/datagrip-alternative", label: "DataGrip alternative" },
  ],
  connections: [
    { href: "/solutions/ssh-database-client", label: "SSH database client guide" },
    { href: "/solutions/postgresql-client", label: "PostgreSQL client guide" },
  ],
  installation: [
    { href: "/download", label: "Download Tabularis" },
    { href: "/solutions/open-source-database-client-linux", label: "Linux database client guide" },
  ],
};

export function getRelatedLinksForPost(tags: string[]): RelatedLink[] {
  const seen = new Set<string>();
  const links: RelatedLink[] = [];
  for (const tag of tags) {
    for (const link of TAG_TO_LINKS[tag] ?? []) {
      if (seen.has(link.href)) continue;
      seen.add(link.href);
      links.push(link);
    }
  }
  return links.slice(0, 4);
}

export function getRelatedLinksForWiki(slug: string): RelatedLink[] {
  return WIKI_TO_LINKS[slug] ?? [];
}
