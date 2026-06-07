import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type { PluginManifest, RegistryPluginWithStatus } from '../types/plugins';
import {
  builtinToCatalogueDriver,
  groupByEngine,
  paradigmFacets,
  toCatalogueDriver,
  type EngineGroup,
  type ParadigmFacet,
} from '../utils/connectionCatalogue';

const BUILTIN_META: Record<string, { engine: string; paradigms: string[] }> = {
  postgres: { engine: 'postgres', paradigms: ['sql'] },
  mysql: { engine: 'mysql', paradigms: ['sql'] },
  sqlite: { engine: 'sqlite', paradigms: ['sql'] },
};

export interface ConnectionCatalogue {
  groups: EngineGroup[];
  facets: ParadigmFacet[];
  loading: boolean;
  registryOffline: boolean;
  refresh: () => void;
}

export function useConnectionCatalogue(): ConnectionCatalogue {
  const [registry, setRegistry] = useState<RegistryPluginWithStatus[]>([]);
  const [builtins, setBuiltins] = useState<PluginManifest[]>([]);
  const [loading, setLoading] = useState(true);
  const [registryOffline, setRegistryOffline] = useState(false);
  const [nonce, setNonce] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void (async () => {
      try {
        const drivers = await invoke<PluginManifest[]>('get_registered_drivers');
        if (!cancelled) setBuiltins(drivers.filter((d) => d.is_builtin === true));
      } catch {
        /* built-ins always have a fallback in useDrivers; ignore here */
      }
      try {
        const cat = await invoke<RegistryPluginWithStatus[]>('fetch_plugin_registry');
        if (!cancelled) {
          setRegistry(cat);
          setRegistryOffline(false);
        }
      } catch {
        if (!cancelled) setRegistryOffline(true);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [nonce]);

  const groups = useMemo(() => {
    const builtinDrivers = builtins.map((m) => {
      const meta = BUILTIN_META[m.id] ?? { engine: m.id, paradigms: [] };
      return builtinToCatalogueDriver(m, meta.engine, meta.paradigms);
    });
    const registryDrivers = registry
      // built-ins are represented from manifests above; skip any registry echo
      .filter((p) => !(p.id in BUILTIN_META))
      .map(toCatalogueDriver);
    return groupByEngine([...builtinDrivers, ...registryDrivers]);
  }, [builtins, registry]);

  const facets = useMemo(() => paradigmFacets(groups), [groups]);
  const refresh = useCallback(() => setNonce((n) => n + 1), []);

  return { groups, facets, loading, registryOffline, refresh };
}
