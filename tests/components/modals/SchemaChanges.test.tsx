import type { ReactNode } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { CreateIndexModal } from '../../../src/components/modals/CreateIndexModal';
import { ModifyColumnModal } from '../../../src/components/modals/ModifyColumnModal';

vi.mock('lucide-react', () => ({ X: () => null, Save: () => null, Loader2: () => null, ListTree: () => null, AlertTriangle: () => null, Columns: () => null, Plus: () => null }));
vi.mock('../../../src/hooks/useDatabase', () => ({ useDatabase: () => ({ activeSchema: 'fixture_db' }) }));
vi.mock('../../../src/hooks/useDrivers', () => ({ useDrivers: () => ({ allDrivers: [{ id: 'mysql', capabilities: { alter_column: true } }] }) }));
vi.mock('../../../src/hooks/useDataTypes', () => {
  const dataTypes = { types: [{ name: 'VARCHAR', requires_length: true, supports_auto_increment: false }] };
  return { useDataTypes: () => ({ dataTypes }) };
});
vi.mock('../../../src/components/ui/Modal', () => ({ Modal: ({ isOpen, children }: { isOpen: boolean; children: ReactNode }) => isOpen ? <div>{children}</div> : null }));
vi.mock('../../../src/components/ui/SqlPreview', () => ({ SqlPreview: ({ sql }: { sql: string }) => <pre>{sql}</pre> }));
vi.mock('../../../src/components/ui/Select', () => ({ Select: () => null }));

const statements = {
  get_create_index_sql: 'CREATE INDEX `idx_orders_note` ON `orders` (`note`)',
  get_add_column_sql: 'ALTER TABLE `orders` ADD COLUMN `note` VARCHAR(255)',
  get_alter_column_sql: 'ALTER TABLE `orders` MODIFY COLUMN `note` VARCHAR(64)',
};

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === 'get_columns') return [{ name: 'note' }];
    if (command in statements) return [statements[command as keyof typeof statements]];
    if (command === 'execute_query') return undefined;
    throw new Error(`Unexpected backend call: ${command}`);
  });
});

const props = () => ({ isOpen: true, onClose: vi.fn(), onSuccess: vi.fn(), connectionId: 'fixture', tableName: 'orders', driver: 'mysql' });

function expectNativeOnly() {
  expect(screen.queryByRole('button', { name: /online/i })).not.toBeInTheDocument();
  expect(vi.mocked(invoke).mock.calls.every(([command]) => command === 'get_columns' || command in statements || command === 'execute_query')).toBe(true);
}

describe('Schema change dialogs', () => {
  it('creates an index through the existing query path and preserves its schema', async () => {
    const callbacks = props();
    render(<CreateIndexModal {...callbacks} />);
    fireEvent.click(await screen.findByRole('checkbox', { name: 'note' }));
    fireEvent.change(screen.getByPlaceholderText('idx_table_column'), { target: { value: 'idx_orders_note' } });
    fireEvent.click(screen.getByRole('button', { name: 'createIndex.create' }));
    await waitFor(() => expect(callbacks.onSuccess).toHaveBeenCalledOnce());
    expect(invoke).toHaveBeenCalledWith('execute_query', { connectionId: 'fixture', query: statements.get_create_index_sql, schema: 'fixture_db' });
    expect(callbacks.onClose).toHaveBeenCalledOnce();
    expectNativeOnly();
  });

  it.each([false, true])('submits a column change through the existing query path (edit=%s)', async edit => {
    const callbacks = props();
    const column = { name: 'note', data_type: 'VARCHAR(64)', is_nullable: true, is_pk: false, is_auto_increment: false };
    const view = render(<ModifyColumnModal {...callbacks} column={edit ? column : null} />);
    if (!edit) fireEvent.change(view.container.querySelector('input')!, { target: { value: 'note' } });
    fireEvent.click(screen.getByRole('button', { name: edit ? 'modifyColumn.save' : 'modifyColumn.add' }));
    await waitFor(() => expect(callbacks.onSuccess).toHaveBeenCalledOnce());
    expect(invoke).toHaveBeenCalledWith('execute_query', { connectionId: 'fixture', query: statements[edit ? 'get_alter_column_sql' : 'get_add_column_sql'], schema: 'fixture_db' });
    expect(callbacks.onClose).toHaveBeenCalledOnce();
    expectNativeOnly();
  });

  it('keeps query failures visible instead of reporting success', async () => {
    const callbacks = props();
    const implementation = vi.mocked(invoke).getMockImplementation()!;
    vi.mocked(invoke).mockImplementation(async (command, args, options) => {
      if (command === 'execute_query') throw new Error('fixture DDL rejected');
      return implementation(command, args, options);
    });
    render(<CreateIndexModal {...callbacks} />);
    fireEvent.click(await screen.findByRole('checkbox', { name: 'note' }));
    fireEvent.click(screen.getByRole('button', { name: 'createIndex.create' }));
    expect(await screen.findByText('Error: fixture DDL rejected')).toBeInTheDocument();
    expect(callbacks.onSuccess).not.toHaveBeenCalled();
    expect(callbacks.onClose).not.toHaveBeenCalled();
    expectNativeOnly();
  });
});
