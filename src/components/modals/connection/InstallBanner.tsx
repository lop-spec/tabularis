import type { CatalogueDriver } from '../../../utils/connectionCatalogue';

export type InstallStatus = 'idle' | 'installing' | 'error';

interface InstallBannerProps {
  driver: CatalogueDriver;
  status: InstallStatus;
  error?: string;
  onInstall: (slug: string, version: string) => void;
}

export function InstallBanner({ driver, status, error, onInstall }: InstallBannerProps) {
  const trigger = () => onInstall(driver.slug, driver.latestVersion);
  return (
    <div className="flex items-center justify-between gap-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2">
      <div className="min-w-0">
        <p className="text-sm font-medium text-primary">Driver not installed</p>
        {status === 'error' && error && <p className="truncate text-xs text-red-400">{error}</p>}
        {status === 'installing' && <p className="text-xs text-muted">Installing {driver.name}…</p>}
      </div>
      {status !== 'installing' && (
        <button
          type="button"
          onClick={trigger}
          disabled={!driver.platformSupported}
          className="shrink-0 cursor-pointer rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {status === 'error' ? 'Retry' : 'Install driver'}
        </button>
      )}
    </div>
  );
}
