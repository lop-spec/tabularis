import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Activity, Loader2, Pause, Play, Square, X } from 'lucide-react';
import { useDatabase } from '../../hooks/useDatabase';
import { Modal } from '../ui/Modal';
import { SqlPreview } from '../ui/SqlPreview';
import { loadOnlineDdlSettings, onlineDdlStatements, replicaEndpoints } from '../../utils/onlineDdl';
import type { OnlineDdlJob, OnlineDdlSettings } from '../../utils/onlineDdl';

interface Props {
  connectionId: string;
  tableName: string;
  disabled?: boolean;
  getStatements: () => Promise<string[]>;
  onSuccess: () => void;
}

interface Session extends OnlineDdlSettings {
  statements: string[];
  database: string;
}

export function OnlineDdlButton({ connectionId, tableName, disabled, getStatements, onSuccess }: Props) {
  const { t } = useTranslation();
  const { connectionDataMap } = useDatabase();
  const data = connectionDataMap[connectionId];
  const databases = [...new Set((data?.selectedDatabases.length ? data.selectedDatabases : [data?.databaseName || '']).filter(Boolean))];
  const [session, setSession] = useState<Session | null>(null);
  const [job, setJob] = useState<OnlineDdlJob | null>(null);
  const [preview, setPreview] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const notified = useRef<string | null>(null);
  const active = job !== null && !job.finished;
  const open = session !== null;

  const receiveJob = useCallback((next: OnlineDdlJob | null) => {
    if (!next) return;
    setJob(next);
    if (next.status === 'succeeded' && notified.current !== next.id) {
      notified.current = next.id;
      onSuccess();
    }
  }, [onSuccess]);

  useEffect(() => {
    if (!open || !active) return;
    let disposed = false;
    let pending = false;
    const poll = async () => {
      if (pending) return;
      pending = true;
      try {
        const next = await invoke<OnlineDdlJob | null>('get_online_ddl_job', { connectionId });
        if (!disposed) receiveJob(next);
      } catch (cause) {
        console.error('Online DDL status unavailable; execution state is not assumed', cause);
        if (!disposed) setError(String(cause));
      } finally { pending = false; }
    };
    const timer = window.setInterval(() => void poll(), 1_000);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [open, active, connectionId, receiveJob]);

  useEffect(() => {
    if (!session || job) return;
    let disposed = false;
    invoke<string>('preview_online_ddl', {
      request: { connectionId, table: tableName, database: session.database, statements: session.statements, replicas: [], aliyunRds: session.aliyunRds },
    }).then(value => { if (!disposed) setPreview(value); })
      .catch(cause => { if (!disposed) { setPreview(''); setError(String(cause)); } });
    return () => { disposed = true; };
  }, [session, job, connectionId, tableName]);

  const launch = async () => {
    setBusy(true); setError(''); setPreview('');
    try {
      if (!await invoke<boolean>('online_ddl_available')) throw new Error(t('onlineDdl.unavailable'));
      const existing = await invoke<OnlineDdlJob | null>('get_online_ddl_job', { connectionId });
      const statements = onlineDdlStatements(await getStatements());
      setJob(existing && !existing.finished ? existing : null);
      setSession({ statements, database: databases.length === 1 ? databases[0] : '', ...loadOnlineDdlSettings(connectionId) });
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };

  const start = async () => {
    if (!session || !preview || busy || active) return;
    setBusy(true); setError('');
    try {
      const replicas = replicaEndpoints(session.replicas);
      const settings: OnlineDdlSettings = { replicas: session.replicas, aliyunRds: session.aliyunRds };
      try { localStorage.setItem(`online-ddl:${connectionId}`, JSON.stringify(settings)); }
      catch (cause) { console.warn('Online DDL settings were not persisted', cause); }
      const next = await invoke<OnlineDdlJob>('start_online_ddl', {
        request: { connectionId, database: session.database, table: tableName, statements: session.statements, replicas, aliyunRds: session.aliyunRds },
      });
      receiveJob(next);
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };

  const control = async (action: 'pause' | 'resume' | 'cancel') => {
    if (!job) return;
    setBusy(true); setError('');
    try {
      await invoke('control_online_ddl', { connectionId, jobId: job.id, action });
      receiveJob(await invoke<OnlineDdlJob | null>('get_online_ddl_job', { connectionId }));
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  };

  const close = () => { if (!active && !busy) { setSession(null); setJob(null); setError(''); } };

  return <>
    <button type="button" onClick={() => void launch()} disabled={disabled || busy || open}
      className="px-3 py-2 rounded-lg text-sm border border-strong text-accent-primary disabled:opacity-50 flex items-center gap-2">
      {busy && !open ? <Loader2 size={16} className="animate-spin" /> : <Activity size={16} />}{t('onlineDdl.button')}
    </button>
    {!open && error && <span role="alert" className="text-xs text-error-text max-w-64">{error}</span>}
    <Modal isOpen={open} onClose={close} overlayClassName="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[150]">
      <div className="bg-elevated border border-strong rounded-xl shadow-2xl w-[720px] max-w-[95vw] max-h-[90vh] flex flex-col overflow-hidden">
        <div className="p-4 border-b border-default flex items-center justify-between bg-base">
          <div><h2 className="text-lg font-semibold text-primary">{t('onlineDdl.title')}</h2><p className="text-xs text-secondary font-mono">{job ? `${job.database}.${job.table}` : `${session?.database || '?'}.${tableName}`}</p></div>
          <button type="button" aria-label={t('common.close')} disabled={active || busy} onClick={close} className="text-secondary disabled:opacity-30"><X size={20} /></button>
        </div>
        <div className="p-5 space-y-4 overflow-y-auto">
          <p className="text-xs text-warning-text bg-warning-bg border border-warning-border p-3 rounded-lg">{t('onlineDdl.warning')}</p>
          {!job && session && <>
            <label className="block text-sm text-secondary">{t('onlineDdl.database')}
              <select aria-label={t('onlineDdl.database')} value={session.database} disabled={busy} onChange={event => { setPreview(''); setError(''); setSession({ ...session, database: event.target.value }); }} className="w-full mt-1 p-2 bg-base border border-strong rounded-lg text-primary">
                <option value="">{t('onlineDdl.selectDatabase')}</option>{databases.map(database => <option key={database} value={database}>{database}</option>)}
              </select>
            </label>
            <label className="block text-sm text-secondary">{t('onlineDdl.replicas')}
              <textarea aria-label={t('onlineDdl.replicas')} value={session.replicas} disabled={busy} onChange={event => setSession({ ...session, replicas: event.target.value })} rows={2} placeholder="replica-a.example.invalid:3306, replica-b.example.invalid:3306" className="w-full mt-1 p-2 bg-base border border-strong rounded-lg text-primary font-mono text-xs" />
            </label>
            <p className="text-xs text-muted">{t('onlineDdl.replicaHelp')}</p>
            <label className="flex gap-2 text-sm text-secondary"><input type="checkbox" checked={session.aliyunRds} disabled={busy} onChange={event => setSession({ ...session, aliyunRds: event.target.checked })} />{t('onlineDdl.aliyun')}</label>
          </>}
          {(job?.sql || preview) && <SqlPreview sql={job?.sql || preview} height="110px" showLineNumbers />}
          {job && <>
            <div role="status" className="text-sm text-primary space-y-2">
              <div>{t(`onlineDdl.states.${job.status}`, { defaultValue: job.status })} · {job.progress === null ? '—' : `${job.progress.toFixed(1)}%`}</div>
              <div>{t('onlineDdl.lag')}: {job.maxReplicaLagMs === null ? t('onlineDdl.unknown') : `${(job.maxReplicaLagMs / 1000).toFixed(2)} s`} · {t('onlineDdl.threshold')}</div>
              {job.reason && <p className="text-xs text-secondary break-words">{job.reason}</p>}
            </div>
            <pre aria-label={t('onlineDdl.logs')} className="bg-base border border-strong rounded-lg p-3 text-xs text-secondary overflow-auto max-h-52 whitespace-pre-wrap break-all">{job.logs.join('\n')}</pre>
          </>}
          {error && <p role="alert" className="text-xs text-error-text break-words">{error}</p>}
        </div>
        <div className="p-4 border-t border-default flex justify-end gap-2 bg-base/50">
          {active ? <>
            <button type="button" disabled={busy || job.status === 'cancelling'} onClick={() => void control('pause')} className="px-3 py-2 text-secondary flex items-center gap-1 disabled:opacity-30"><Pause size={14} />{t('onlineDdl.pause')}</button>
            <button type="button" disabled={busy || job.status === 'cancelling'} onClick={() => void control('resume')} className="px-3 py-2 text-secondary flex items-center gap-1 disabled:opacity-30"><Play size={14} />{t('onlineDdl.resume')}</button>
            <button type="button" disabled={busy || job.status === 'cancelling'} onClick={() => void control('cancel')} className="px-3 py-2 text-red-400 flex items-center gap-1 disabled:opacity-30"><Square size={14} />{t('onlineDdl.cancel')}</button>
          </> : <>
            <button type="button" disabled={busy} onClick={close} className="px-3 py-2 text-secondary">{t('common.close')}</button>
            {!job && <button type="button" disabled={busy || !preview || !session?.database || !session?.replicas.trim()} onClick={() => void start()} className="px-4 py-2 bg-blue-600 text-white rounded-lg disabled:opacity-40">{busy ? t('common.loading') : t('onlineDdl.execute')}</button>}
          </>}
        </div>
      </div>
    </Modal>
  </>;
}
