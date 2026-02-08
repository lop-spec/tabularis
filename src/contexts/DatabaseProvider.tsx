import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DatabaseContext, type TableInfo, type ViewInfo, type RoutineInfo, type SavedConnection } from './DatabaseContext';
import type { ReactNode } from 'react';
import { clearAutocompleteCache } from '../utils/autocomplete';

export const DatabaseProvider = ({ children }: { children: ReactNode }) => {
  const [activeConnectionId, setActiveConnectionId] = useState<string | null>(null);
  const [activeDriver, setActiveDriver] = useState<string | null>(null);
  const [activeTable, setActiveTable] = useState<string | null>(null);
  const [activeConnectionName, setActiveConnectionName] = useState<string | null>(null);
  const [activeDatabaseName, setActiveDatabaseName] = useState<string | null>(null);
  const [tables, setTables] = useState<TableInfo[]>([]);
  const [views, setViews] = useState<ViewInfo[]>([]);
  const [routines, setRoutines] = useState<RoutineInfo[]>([]);
  const [isLoadingTables, setIsLoadingTables] = useState(false);
  const [isLoadingViews, setIsLoadingViews] = useState(false);
  const [isLoadingRoutines, setIsLoadingRoutines] = useState(false);

  // Sync Window Title with active connection
  // WORKAROUND: Using custom Tauri command instead of window.setTitle() for Wayland support
  // See: https://github.com/tauri-apps/tauri/issues/13749
  useEffect(() => {
    const updateTitle = async () => {
      try {
        const title = (activeConnectionName && activeDatabaseName)
          ? `tabularis - ${activeConnectionName} (${activeDatabaseName})`
          : 'tabularis';
        await invoke('set_window_title', { title });
      } catch (e) {
        console.error('Failed to update window title', e);
      }
    };
    updateTitle();
  }, [activeConnectionName, activeDatabaseName]);

  const refreshTables = async () => {
      if (!activeConnectionId) return;
      setIsLoadingTables(true);
      try {
          const result = await invoke<TableInfo[]>('get_tables', { connectionId: activeConnectionId });
          setTables(result);
      } catch (e) {
          console.error('Failed to refresh tables:', e);
      } finally {
          setIsLoadingTables(false);
      }
  };

  const refreshViews = async () => {
      if (!activeConnectionId) return;
      setIsLoadingViews(true);
      try {
          const result = await invoke<ViewInfo[]>('get_views', { connectionId: activeConnectionId });
          setViews(result);
      } catch (e) {
          console.error('Failed to refresh views:', e);
      } finally {
          setIsLoadingViews(false);
      }
  };

  const refreshRoutines = async () => {
    if (!activeConnectionId) return;
    setIsLoadingRoutines(true);
    try {
      const result = await invoke<RoutineInfo[]>('get_routines', { connectionId: activeConnectionId });
      setRoutines(result);
    } catch (e) {
      console.error('Failed to refresh routines:', e);
    } finally {
      setIsLoadingRoutines(false);
    }
  };

  const connect = async (connectionId: string) => {
    setActiveConnectionId(connectionId);
    setIsLoadingTables(true);
    setIsLoadingViews(true);
    setIsLoadingRoutines(true);
    setTables([]);
    setViews([]);
    setRoutines([]);
    setActiveDriver(null);
    setActiveTable(null);
    setActiveConnectionName(null);
    setActiveDatabaseName(null);

    try {
      // 1. Get driver info
      const connections = await invoke<SavedConnection[]>('get_connections');
      const conn = connections.find(c => c.id === connectionId);
      if (conn) {
        setActiveDriver(conn.params.driver);
        setActiveConnectionName(conn.name);
        setActiveDatabaseName(conn.params.database);
      }

      // 2. Get tables and views in parallel
      const [tablesResult, viewsResult, routinesResult] = await Promise.all([
        invoke<TableInfo[]>('get_tables', { connectionId }),
        invoke<ViewInfo[]>('get_views', { connectionId }),
        invoke<RoutineInfo[]>('get_routines', { connectionId })
      ]);
      setTables(tablesResult);
      setViews(viewsResult);
      setRoutines(routinesResult);
    } catch (error) {
      console.error('Failed to fetch tables/views/routines:', error);
      setActiveConnectionId(null);
      setActiveDriver(null);
      setActiveConnectionName(null);
      setActiveDatabaseName(null);
      throw error;
    } finally {
      setIsLoadingTables(false);
      setIsLoadingViews(false);
      setIsLoadingRoutines(false);
    }
  };

  const disconnect = () => {
    // Clear autocomplete cache for this connection
    if (activeConnectionId) {
      clearAutocompleteCache(activeConnectionId);
    }

    setActiveConnectionId(null);
    setActiveDriver(null);
    setActiveTable(null);
    setActiveConnectionName(null);
    setActiveDatabaseName(null);
    setTables([]);
    setViews([]);
    setRoutines([]);
  };

  return (
    <DatabaseContext.Provider value={{
      activeConnectionId,
      activeDriver,
      activeTable,
      activeConnectionName,
      activeDatabaseName,
      tables,
      views,
      routines,
      isLoadingTables,
      isLoadingViews,
      isLoadingRoutines,
      connect,
      disconnect,
      setActiveTable,
      refreshTables,
      refreshViews,
      refreshRoutines
    }}>
      {children}
    </DatabaseContext.Provider>
  );
};
