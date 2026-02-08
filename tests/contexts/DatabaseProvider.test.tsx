import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { DatabaseProvider } from '../../src/contexts/DatabaseProvider';
import { useDatabase } from '../../src/hooks/useDatabase';
import { invoke } from '@tauri-apps/api/core';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../src/utils/autocomplete', () => ({
  clearAutocompleteCache: vi.fn(),
}));

describe('DatabaseProvider', () => {
  const mockConnections = [
    {
      id: 'conn-123',
      name: 'Local MySQL',
      params: {
        driver: 'mysql',
        host: 'localhost',
        database: 'testdb',
      },
    },
  ];

  const mockTables = [
    { name: 'users' },
    { name: 'posts' },
    { name: 'comments' },
  ];

  const mockViews = [
    { name: 'active_users' },
    { name: 'user_posts_summary' },
  ];

  beforeEach(() => {
    vi.resetAllMocks();
    // Default mock implementation that handles all invoke calls
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_connections') return Promise.resolve(mockConnections);
      if (cmd === 'get_tables') return Promise.resolve(mockTables);
      if (cmd === 'get_views') return Promise.resolve(mockViews);
      if (cmd === 'set_window_title') return Promise.resolve(undefined);
      return Promise.reject(new Error(`Unexpected command: ${cmd}`));
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should provide initial state', () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    expect(result.current.activeConnectionId).toBeNull();
    expect(result.current.activeDriver).toBeNull();
    expect(result.current.activeTable).toBeNull();
    expect(result.current.tables).toHaveLength(0);
    expect(result.current.views).toHaveLength(0);
    expect(result.current.isLoadingTables).toBe(false);
    expect(result.current.isLoadingViews).toBe(false);
  });

  it('should connect and load tables', async () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await act(async () => {
      await result.current.connect('conn-123');
    });

    await waitFor(() => {
      expect(result.current.activeConnectionId).toBe('conn-123');
      expect(result.current.activeDriver).toBe('mysql');
      expect(result.current.activeConnectionName).toBe('Local MySQL');
      expect(result.current.activeDatabaseName).toBe('testdb');
      expect(result.current.tables).toHaveLength(3);
      expect(result.current.views).toHaveLength(2);
      expect(result.current.isLoadingTables).toBe(false);
      expect(result.current.isLoadingViews).toBe(false);
    });

    expect(invoke).toHaveBeenCalledWith('get_connections');
    expect(invoke).toHaveBeenCalledWith('get_tables', { connectionId: 'conn-123' });
    expect(invoke).toHaveBeenCalledWith('get_views', { connectionId: 'conn-123' });
  });

  it('should handle connection failure', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('Connection failed'));

    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await expect(result.current.connect('conn-123')).rejects.toThrow('Connection failed');

    await waitFor(() => {
      expect(result.current.activeConnectionId).toBeNull();
      expect(result.current.activeDriver).toBeNull();
      expect(result.current.isLoadingTables).toBe(false);
    });
  });

  it('should disconnect and reset state', async () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await act(async () => {
      await result.current.connect('conn-123');
    });
    
    await waitFor(() => expect(result.current.activeConnectionId).toBe('conn-123'));

    act(() => {
      result.current.disconnect();
    });

    expect(result.current.activeConnectionId).toBeNull();
    expect(result.current.activeDriver).toBeNull();
    expect(result.current.activeTable).toBeNull();
    expect(result.current.tables).toHaveLength(0);
    expect(result.current.views).toHaveLength(0);
  });

  it('should refresh tables', async () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await act(async () => {
      await result.current.connect('conn-123');
    });
    
    await waitFor(() => expect(result.current.tables).toHaveLength(3));

    // Update mock to return additional table
    const updatedTables = [...mockTables, { name: 'new_table' }];
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_connections') return Promise.resolve(mockConnections);
      if (cmd === 'get_tables') return Promise.resolve(updatedTables);
      if (cmd === 'set_window_title') return Promise.resolve(undefined);
      return Promise.reject(new Error(`Unexpected command: ${cmd}`));
    });

    await act(async () => {
      await result.current.refreshTables();
    });

    await waitFor(() => {
      expect(result.current.tables).toHaveLength(4);
    });
    expect(result.current.tables.map((t) => t.name)).toContain('new_table');
  });

  it('should not refresh tables when disconnected', async () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await act(async () => {
      await result.current.refreshTables();
    });

    // Should not invoke get_tables when disconnected
    expect(invoke).not.toHaveBeenCalledWith('get_tables', expect.anything());
  });

  it('should set active table', async () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await act(async () => {
      await result.current.connect('conn-123');
    });
    
    await waitFor(() => expect(result.current.activeConnectionId).toBe('conn-123'));

    act(() => {
      result.current.setActiveTable('users');
    });

    expect(result.current.activeTable).toBe('users');

    act(() => {
      result.current.setActiveTable(null);
    });

    expect(result.current.activeTable).toBeNull();
  });

  it('should update window title on connection', async () => {
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(DatabaseProvider, null, children);

    const { result } = renderHook(() => useDatabase(), { wrapper });

    await act(async () => {
      await result.current.connect('conn-123');
    });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('set_window_title', {
        title: 'tabularis - Local MySQL (testdb)',
      });
    });
  });

  describe('Views Management', () => {
    it('should load views on connection', async () => {
      const wrapper = ({ children }: { children: React.ReactNode }) =>
        React.createElement(DatabaseProvider, null, children);

      const { result } = renderHook(() => useDatabase(), { wrapper });

      await act(async () => {
        await result.current.connect('conn-123');
      });

      await waitFor(() => {
        expect(result.current.views).toHaveLength(2);
        expect(result.current.views[0].name).toBe('active_users');
        expect(result.current.views[1].name).toBe('user_posts_summary');
        expect(result.current.isLoadingViews).toBe(false);
      });
    });

    it('should refresh views', async () => {
      const wrapper = ({ children }: { children: React.ReactNode }) =>
        React.createElement(DatabaseProvider, null, children);

      const { result } = renderHook(() => useDatabase(), { wrapper });

      await act(async () => {
        await result.current.connect('conn-123');
      });

      await waitFor(() => expect(result.current.views).toHaveLength(2));

      // Update mock to return additional view
      const updatedViews = [...mockViews, { name: 'new_view' }];
      vi.mocked(invoke).mockImplementation((cmd: string) => {
        if (cmd === 'get_connections') return Promise.resolve(mockConnections);
        if (cmd === 'get_tables') return Promise.resolve(mockTables);
        if (cmd === 'get_views') return Promise.resolve(updatedViews);
        if (cmd === 'set_window_title') return Promise.resolve(undefined);
        return Promise.reject(new Error(`Unexpected command: ${cmd}`));
      });

      await act(async () => {
        await result.current.refreshViews();
      });

      await waitFor(() => {
        expect(result.current.views).toHaveLength(3);
      });
      expect(result.current.views.map((v) => v.name)).toContain('new_view');
    });

    it('should not refresh views when disconnected', async () => {
      const wrapper = ({ children }: { children: React.ReactNode }) =>
        React.createElement(DatabaseProvider, null, children);

      const { result } = renderHook(() => useDatabase(), { wrapper });

      await act(async () => {
        await result.current.refreshViews();
      });

      // Should not invoke get_views when disconnected
      expect(invoke).not.toHaveBeenCalledWith('get_views', expect.anything());
    });

    it('should handle views loading state during connection', async () => {
      const wrapper = ({ children }: { children: React.ReactNode }) =>
        React.createElement(DatabaseProvider, null, children);

      const { result } = renderHook(() => useDatabase(), { wrapper });

      // Start connection
      act(() => {
        result.current.connect('conn-123');
      });

      // Should be loading immediately after starting connection
      expect(result.current.isLoadingViews).toBe(true);

      await waitFor(() => {
        expect(result.current.isLoadingViews).toBe(false);
      });
    });

    it('should clear views on disconnect', async () => {
      const wrapper = ({ children }: { children: React.ReactNode }) =>
        React.createElement(DatabaseProvider, null, children);

      const { result } = renderHook(() => useDatabase(), { wrapper });

      await act(async () => {
        await result.current.connect('conn-123');
      });

      await waitFor(() => expect(result.current.views).toHaveLength(2));

      act(() => {
        result.current.disconnect();
      });

      expect(result.current.views).toHaveLength(0);
    });
  });
});
