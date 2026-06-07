import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { InstallBanner } from '../../../src/components/modals/connection/InstallBanner';

const driver = { slug: 'qdrant', name: 'Qdrant', engine: 'qdrant', paradigms: ['vector'], verified: false, installed: false, installedVersion: null, latestVersion: '1.3.0', isBuiltin: false, platformSupported: true, downloads: 5, updateAvailable: false, icon: null, color: null };

describe('InstallBanner', () => {
  it('shows the install button when idle', () => {
    render(<InstallBanner driver={driver} status="idle" onInstall={vi.fn()} />);
    expect(screen.getByRole('button', { name: /install driver/i })).toBeInTheDocument();
  });

  it('calls onInstall with slug + latest version', () => {
    const onInstall = vi.fn();
    render(<InstallBanner driver={driver} status="idle" onInstall={onInstall} />);
    screen.getByRole('button', { name: /install driver/i }).click();
    expect(onInstall).toHaveBeenCalledWith('qdrant', '1.3.0');
  });

  it('shows a retry affordance on error', () => {
    render(<InstallBanner driver={driver} status="error" error="boom" onInstall={vi.fn()} />);
    expect(screen.getByText(/boom/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });
});
