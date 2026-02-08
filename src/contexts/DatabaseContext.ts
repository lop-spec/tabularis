import { createContext } from 'react';

export interface TableInfo {
  name: string;
}

export interface ViewInfo {
  name: string;
  definition?: string;
}

export interface RoutineInfo {
  name: string;
  routine_type: string; // "PROCEDURE" | "FUNCTION"
  definition?: string;
}

export interface SavedConnection {
  id: string;
  name: string;
  params: {
    driver: string;
    host?: string;
    database: string;
  };
}

export interface DatabaseContextType {
  activeConnectionId: string | null;
  activeDriver: string | null;
  activeTable: string | null;
  activeConnectionName: string | null;
  activeDatabaseName: string | null;
  tables: TableInfo[];
  views: ViewInfo[];
  routines: RoutineInfo[];
  isLoadingTables: boolean;
  isLoadingViews: boolean;
  isLoadingRoutines: boolean;
  connect: (connectionId: string) => Promise<void>;
  disconnect: () => void;
  setActiveTable: (table: string | null) => void;
  refreshTables: () => Promise<void>;
  refreshViews: () => Promise<void>;
  refreshRoutines: () => Promise<void>;
}

export const DatabaseContext = createContext<DatabaseContextType | undefined>(undefined);
