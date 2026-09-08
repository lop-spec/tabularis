import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { OnlineDdlButton } from '../../../src/components/modals/OnlineDdlButton';
import type { OnlineDdlJob } from '../../../src/utils/onlineDdl';

vi.mock('lucide-react', () => ({ Activity: () => null, Loader2: () => null, Pause: () => null, Play: () => null, Square: () => null, X: () => null }));
vi.mock('../../../src/hooks/useDatabase', () => ({ useDatabase: () => ({ connectionDataMap: { fixture: { selectedDatabases: ['fixture_db'], databaseName: 'fixture_db' } } }) }));
vi.mock('../../../src/components/ui/SqlPreview', () => ({ SqlPreview: ({ sql }: { sql: string }) => <pre>{sql}</pre> }));

const running: OnlineDdlJob = { id: 'job-1', connectionId: 'fixture', database: 'fixture_db', table: 'orders', status: 'running', reason: '', progress: 12, maxReplicaLagMs: 100, finished: false, sql: 'ALTER TABLE `fixture_db`.`orders` ADD INDEX `idx_a` (`a`);', resultDdl: null, logs: ['Copy: 12/100 12.0%;'] };
let current: OnlineDdlJob | null;
let available: boolean;

beforeEach(() => {
  current = null;
  available = true;
  localStorage.clear();
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'online_ddl_available') return available;
    if (command === 'preview_online_ddl') return running.sql;
    if (command === 'get_online_ddl_job') return current;
    if (command === 'start_online_ddl') { current = { ...running }; return current; }
    if (command === 'control_online_ddl') {
      const action = (args as { action: string }).action;
      if (current) current = { ...current, status: action === 'cancel' ? 'cancelling' : action === 'pause' ? 'throttled' : 'running' };
      return undefined;
    }
    throw new Error(`Unexpected backend call ${command}`);
  });
});

const renderButton = () => render(<OnlineDdlButton connectionId="fixture" tableName="orders" getStatements={async () => ['CREATE INDEX `idx_a` ON `orders` (`a`)']} onSuccess={vi.fn()} />);

async function openForm() {
  renderButton();
  fireEvent.click(screen.getByRole('button', { name: 'onlineDdl.button' }));
  await screen.findByText('onlineDdl.title');
  await screen.findByText(running.sql);
}

describe('OnlineDdlButton', () => {
  it('requires replicas, confirms the exact scope, and uses only the OSC command', async () => {
    await openForm();
    const execute = screen.getByRole('button', { name: 'onlineDdl.execute' });
    expect(execute).toBeDisabled();
    fireEvent.change(screen.getByLabelText('onlineDdl.replicas'), { target: { value: 'replica.example.invalid:3306' } });
    fireEvent.click(execute);
    await screen.findByText(/onlineDdl.states.running/);
    expect(invoke).toHaveBeenCalledWith('start_online_ddl', { request: { connectionId: 'fixture', database: 'fixture_db', table: 'orders', statements: ['CREATE INDEX `idx_a` ON `orders` (`a`)'], replicas: ['replica.example.invalid:3306'], aliyunRds: false } });
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'execute_query')).toBe(false);
    expect(screen.getByRole('button', { name: 'common.close' })).toBeDisabled();
  });

  it('routes pause, continue and cancel to the exact active job', async () => {
    current = { ...running };
    await openForm();
    for (const [label, action] of [['onlineDdl.pause', 'pause'], ['onlineDdl.resume', 'resume'], ['onlineDdl.cancel', 'cancel']]) {
      const button = screen.getByRole('button', { name: label });
      await waitFor(() => expect(button).not.toBeDisabled());
      fireEvent.click(button);
      await waitFor(() => expect(invoke).toHaveBeenCalledWith('control_online_ddl', { connectionId: 'fixture', jobId: 'job-1', action }));
    }
    await screen.findByText(/onlineDdl.states.cancelling/);
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'start_online_ddl')).toBe(false);
  });

  it('does not fall back to native DDL when the engine is missing', async () => {
    available = false;
    renderButton();
    fireEvent.click(screen.getByRole('button', { name: 'onlineDdl.button' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('onlineDdl.unavailable');
    expect(screen.queryByText('onlineDdl.title')).not.toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'execute_query' || command === 'start_online_ddl')).toBe(false);
  });
});
