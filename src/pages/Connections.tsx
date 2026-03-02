import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { NewConnectionModal } from '../components/ui/NewConnectionModal';
import { invoke } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';
import {
  Database,
  Plus,
  Power,
  Edit,
  Trash2,
  Shield,
  AlertCircle,
  Copy,
  Loader2,
  PlugZap,
  Search,
  X,
} from 'lucide-react';
import { getDriverColor, getDriverIcon } from '../utils/driverUI';
import { useDatabase } from '../hooks/useDatabase';
import { useDrivers } from '../hooks/useDrivers';
import clsx from 'clsx';
import { isLocalDriver, getCapabilitiesForDriver } from '../utils/driverCapabilities';

interface SavedConnection {
  id: string;
  name: string;
  params: {
    driver: string;
    host?: string;
    database: string | string[];
    port?: number;
    username?: string;
    password?: string;
    ssh_enabled?: boolean;
    ssh_host?: string;
    ssh_port?: number;
    ssh_user?: string;
    ssh_password?: string;
    ssh_key_file?: string;
  };
}


function connectionSubtitle(conn: SavedConnection, capabilities: ReturnType<typeof getCapabilitiesForDriver>): string {
  if (isLocalDriver(capabilities)) {
    const db = conn.params.database;
    return Array.isArray(db) ? db[0] ?? '' : db;
  }
  const db = conn.params.database;
  const dbStr = Array.isArray(db) ? `${db.length} databases` : db;
  return `${conn.params.host ?? 'localhost'}:${conn.params.port ?? ''}  ·  ${dbStr}`;
}

export const Connections = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { connect, activeConnectionId, disconnect, isConnectionOpen, switchConnection } = useDatabase();
  const { drivers, allDrivers } = useDrivers();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingConnection, setEditingConnection] = useState<SavedConnection | null>(null);
  const [connections, setConnections] = useState<SavedConnection[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [connectingId, setConnectingId] = useState<string | null>(null);
  const [search, setSearch] = useState('');

  const loadConnections = async () => {
    try {
      const result = await invoke<SavedConnection[]>('get_connections');
      setConnections(result);
    } catch (e) {
      console.error('Failed to load connections:', e);
    }
  };

  useEffect(() => { void loadConnections(); }, []);

  const handleSave = () => {
    loadConnections();
    setIsModalOpen(false);
    setEditingConnection(null);
  };

  const handleConnect = async (conn: SavedConnection) => {
    setError(null);
    if (isConnectionOpen(conn.id)) {
      switchConnection(conn.id);
      navigate('/editor');
      return;
    }
    setConnectingId(conn.id);
    try {
      await connect(conn.id);
      navigate('/editor');
    } catch (e) {
      const msg = typeof e === 'string' ? e : (e as Error).message || String(e);
      setError(`${t('connections.failConnect', { name: conn.name })}\n\nError: ${msg}`);
    } finally {
      setConnectingId(null);
    }
  };

  const handleDisconnect = async (connId: string) => {
    setError(null);
    try {
      await disconnect(connId);
    } catch (e) {
      const msg = typeof e === 'string' ? e : (e as Error).message || String(e);
      setError(`${t('connections.failDisconnect')}\n\nError: ${msg}`);
    }
  };

  const handleDelete = async (id: string) => {
    const confirmed = await ask(t('connections.confirmDelete'), {
      title: t('connections.deleteTitle'),
      kind: 'warning',
    });
    if (confirmed) {
      try {
        if (isConnectionOpen(id)) await disconnect(id);
        await invoke('delete_connection', { id });
        loadConnections();
      } catch (e) {
        console.error(e);
      }
    }
  };

  const openEdit = async (conn: SavedConnection) => {
    if (isConnectionOpen(conn.id)) {
      await disconnect(conn.id);
    }
    setEditingConnection(conn);
    setIsModalOpen(true);
  };

  const handleDuplicate = async (id: string) => {
    try {
      const newConn = await invoke<SavedConnection>('duplicate_connection', { id });
      await loadConnections();
      openEdit(newConn);
    } catch (e) {
      console.error(e);
      setError(t('connections.failDuplicate'));
    }
  };

  const filtered = search.trim()
    ? connections.filter(c => c.name.toLowerCase().includes(search.toLowerCase()) || c.params.driver.toLowerCase().includes(search.toLowerCase()))
    : connections;

  return (
    <div className="h-full flex flex-col overflow-hidden bg-base">
      {/* Header */}
      <div className="flex items-center justify-between px-8 py-5 border-b border-default bg-elevated shrink-0">
        <div>
          <h1 className="text-lg font-bold text-primary">{t('connections.title')}</h1>
          <p className="text-xs text-muted mt-0.5">
            {connections.length > 0
              ? t('connections.connectionCount', { count: connections.length, defaultValue: '{{count}} connection(s)' })
              : t('connections.noConnections')}
          </p>
        </div>
        <button
          onClick={() => { setEditingConnection(null); setIsModalOpen(true); }}
          className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-3.5 py-2 rounded-lg font-medium text-sm transition-colors"
        >
          <Plus size={15} />
          {t('connections.addConnection')}
        </button>
      </div>

      {/* Error banner */}
      {error && (
        <div className="mx-8 mt-4 p-3 bg-red-900/20 border border-red-900/50 rounded-lg flex items-start gap-3 text-red-400 shrink-0">
          <AlertCircle size={16} className="mt-0.5 shrink-0" />
          <span className="text-sm whitespace-pre-wrap flex-1">{error}</span>
          <button onClick={() => setError(null)} className="text-red-400/60 hover:text-red-400 transition-colors shrink-0">
            <X size={14} />
          </button>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-8 py-5">
        {connections.length === 0 ? (
          /* Empty state */
          <div className="flex flex-col items-center justify-center h-full min-h-[300px] text-center">
            <div className="w-16 h-16 rounded-2xl bg-elevated border border-default flex items-center justify-center mb-4">
              <Database size={28} className="text-muted" />
            </div>
            <p className="text-base font-semibold text-primary mb-1">{t('connections.noConnections')}</p>
            <p className="text-sm text-muted mb-5">{t('connections.noConnectionsHint', { defaultValue: 'Create your first connection to get started.' })}</p>
            <button
              onClick={() => { setEditingConnection(null); setIsModalOpen(true); }}
              className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded-lg font-medium text-sm transition-colors"
            >
              <Plus size={14} />
              {t('connections.createFirst')}
            </button>
          </div>
        ) : (
          <>
            {/* Search */}
            <div className="relative mb-5">
              <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted pointer-events-none" />
              <input
                type="text"
                value={search}
                onChange={e => setSearch(e.target.value)}
                placeholder={t('connections.searchPlaceholder', { defaultValue: 'Search connections...' })}
                className="w-full pl-9 pr-9 py-2.5 bg-elevated border border-strong rounded-lg text-sm text-primary placeholder:text-muted focus:border-blue-500 focus:outline-none transition-colors"
              />
              {search && (
                <button
                  onClick={() => setSearch('')}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted hover:text-primary transition-colors"
                >
                  <X size={13} />
                </button>
              )}
            </div>

            {/* Connection list */}
            <div className="flex flex-col gap-2">
              {filtered.map(conn => {
                const isActive = activeConnectionId === conn.id;
                const isOpen = isConnectionOpen(conn.id);
                const isConnecting = connectingId === conn.id;
                const capabilities = getCapabilitiesForDriver(conn.params.driver, allDrivers);
                const driverManifest = allDrivers.find(d => d.id === conn.params.driver);
                const isDriverEnabled = drivers.some(d => d.id === conn.params.driver);
                const subtitle = connectionSubtitle(conn, capabilities);

                return (
                  <div
                    key={conn.id}
                    onDoubleClick={() => isDriverEnabled && !isConnecting && handleConnect(conn)}
                    className={clsx(
                      'group relative flex items-center gap-3 px-4 py-3.5 rounded-xl border transition-all cursor-pointer select-none',
                      !isDriverEnabled && 'opacity-60 cursor-not-allowed',
                      isConnecting && 'pointer-events-none',
                      isActive
                        ? 'border-blue-500/50 bg-blue-500/8'
                        : isOpen
                          ? 'border-green-500/40 bg-green-500/6 hover:bg-green-500/10'
                          : 'border-strong bg-elevated hover:bg-surface-primary hover:border-strong',
                    )}
                  >
                    {/* Left accent bar for active/open */}
                    {(isActive || isOpen) && (
                      <div className={clsx(
                        'absolute left-0 top-3 bottom-3 w-0.5 rounded-full',
                        isActive ? 'bg-blue-400' : 'bg-green-400/70',
                      )} />
                    )}

                    {/* Driver icon */}
                    <div
                      className="w-9 h-9 rounded-lg flex items-center justify-center text-white shrink-0"
                      style={{ backgroundColor: getDriverColor(driverManifest) }}
                    >
                      {getDriverIcon(driverManifest)}
                    </div>

                    {/* Name + info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-1">
                        <span className="font-semibold text-sm text-primary truncate">{conn.name}</span>
                        <span className="text-[10px] font-medium text-secondary bg-surface-secondary px-1.5 py-0.5 rounded-md capitalize shrink-0 border border-strong/50">
                          {conn.params.driver}
                        </span>
                        {conn.params.ssh_enabled && (
                          <span className="flex items-center gap-0.5 text-[10px] font-semibold text-emerald-400 bg-emerald-400/10 px-1.5 py-0.5 rounded-md shrink-0">
                            <Shield size={9} /> SSH
                          </span>
                        )}
                        {!isDriverEnabled && (
                          <span className="flex items-center gap-1 text-[10px] text-amber-400 bg-amber-400/10 border border-amber-400/20 px-1.5 py-0.5 rounded-md shrink-0">
                            <PlugZap size={9} /> {t('connections.pluginDisabled')}
                          </span>
                        )}
                      </div>
                      <span className="text-xs text-secondary truncate block leading-relaxed">{subtitle}</span>
                    </div>

                    {/* Status badge */}
                    <div className="shrink-0">
                      {isConnecting ? (
                        <Loader2 size={15} className="animate-spin text-blue-400" />
                      ) : isActive ? (
                        <span className="flex items-center gap-1.5 text-[11px] font-semibold text-green-400 bg-green-400/12 border border-green-400/25 px-2.5 py-1 rounded-full">
                          <span className="w-1.5 h-1.5 rounded-full bg-green-400 animate-pulse" />
                          {t('connections.active')}
                        </span>
                      ) : isOpen ? (
                        <span className="flex items-center gap-1.5 text-[11px] font-medium text-green-400/80 bg-green-400/8 border border-green-400/20 px-2.5 py-1 rounded-full">
                          <span className="w-1.5 h-1.5 rounded-full bg-green-400/70" />
                          {t('connections.open')}
                        </span>
                      ) : null}
                    </div>

                    {/* Actions — always visible at low opacity, full on hover */}
                    <div className="flex items-center gap-0.5 opacity-40 group-hover:opacity-100 transition-opacity shrink-0">
                      {isOpen ? (
                        <button
                          onClick={e => { e.stopPropagation(); handleDisconnect(conn.id); }}
                          className="p-1.5 rounded-lg text-muted hover:text-red-400 hover:bg-red-400/10 transition-colors"
                          title={t('connections.disconnect')}
                        >
                          <Power size={14} />
                        </button>
                      ) : (
                        <button
                          onClick={e => { e.stopPropagation(); if (isDriverEnabled) handleConnect(conn); }}
                          disabled={!isDriverEnabled}
                          className="p-1.5 rounded-lg text-muted hover:text-green-400 hover:bg-green-400/10 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                          title={isDriverEnabled ? t('connections.connect') : t('connections.pluginDisabled')}
                        >
                          <Power size={14} />
                        </button>
                      )}
                      <button
                        onClick={e => { e.stopPropagation(); if (isDriverEnabled) void openEdit(conn); }}
                        disabled={!isDriverEnabled}
                        className="p-1.5 rounded-lg text-muted hover:text-blue-400 hover:bg-blue-400/10 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                        title={t('connections.edit')}
                      >
                        <Edit size={14} />
                      </button>
                      <button
                        onClick={e => { e.stopPropagation(); if (isDriverEnabled) handleDuplicate(conn.id); }}
                        disabled={!isDriverEnabled}
                        className="p-1.5 rounded-lg text-muted hover:text-purple-400 hover:bg-purple-400/10 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                        title={t('connections.clone')}
                      >
                        <Copy size={14} />
                      </button>
                      <button
                        onClick={e => { e.stopPropagation(); handleDelete(conn.id); }}
                        className="p-1.5 rounded-lg text-muted hover:text-red-400 hover:bg-red-400/10 transition-colors"
                        title={t('connections.delete')}
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </div>
                );
              })}

              {filtered.length === 0 && search && (
                <div className="text-center py-10 text-sm text-muted">
                  {t('connections.noSearchResults', { defaultValue: 'No connections match "{{query}}"', query: search })}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      <NewConnectionModal
        isOpen={isModalOpen}
        onClose={() => { setIsModalOpen(false); setEditingConnection(null); }}
        onSave={handleSave}
        initialConnection={editingConnection}
      />
    </div>
  );
};
