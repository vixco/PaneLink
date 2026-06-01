import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';

export type UpdateStatus =
  | { state: 'idle'; label: string }
  | { state: 'checking'; label: string }
  | { state: 'current'; label: string }
  | { state: 'available'; label: string }
  | { state: 'installing'; label: string }
  | { state: 'error'; label: string };

const isTauri = '__TAURI_INTERNALS__' in window;

export async function checkAndInstallUpdate(onStatus: (status: UpdateStatus) => void) {
  if (!isTauri) {
    onStatus({ state: 'current', label: 'Preview build' });
    return;
  }

  try {
    onStatus({ state: 'checking', label: 'Checking updates' });
    const update = await check();

    if (!update) {
      onStatus({ state: 'current', label: 'Up to date' });
      return;
    }

    onStatus({ state: 'available', label: `Update ${update.version}` });
    await update.downloadAndInstall();
    onStatus({ state: 'installing', label: 'Restarting' });
    await relaunch();
  } catch (error) {
    console.warn('PaneLink updater check failed', error);
    onStatus({ state: 'error', label: 'Update check failed' });
  }
}
