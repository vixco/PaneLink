import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';

export type UpdateStatus =
  | { state: 'idle'; label: string }
  | { state: 'checking'; label: string }
  | { state: 'current'; label: string }
  | { state: 'available'; label: string }
  | { state: 'downloading'; label: string }
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
    let downloadedBytes = 0;
    let totalBytes: number | undefined;

    await update.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        downloadedBytes = 0;
        totalBytes = event.data.contentLength;
        onStatus({ state: 'downloading', label: 'Downloading update' });
      }

      if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength;
        if (totalBytes) {
          const percent = Math.min(99, Math.round((downloadedBytes / totalBytes) * 100));
          onStatus({ state: 'downloading', label: `Downloading ${percent}%` });
        } else {
          onStatus({ state: 'downloading', label: 'Downloading update' });
        }
      }

      if (event.event === 'Finished') {
        onStatus({ state: 'installing', label: 'Installing update' });
      }
    });

    onStatus({ state: 'installing', label: 'Restarting app' });
    await relaunch();
  } catch (error) {
    console.warn('PaneLink updater check failed', error);
    onStatus({ state: 'error', label: updateErrorLabel(error) });
  }
}

function updateErrorLabel(error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  if (/platform|target/i.test(message)) {
    return 'Update package missing for this device';
  }
  if (/signature|verify/i.test(message)) {
    return 'Update signature failed';
  }
  if (/network|fetch|request|download/i.test(message)) {
    return 'Update download failed';
  }
  return message && message !== 'undefined' ? `Update failed: ${message}` : 'Update failed';
}
