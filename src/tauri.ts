import { invoke } from '@tauri-apps/api/core';
import { fallbackCapabilities, fallbackDevices, fallbackPeers, fallbackPermissions, fallbackSession } from './fixtures';
import type { AudioDevice, Capabilities, Peer, PermissionState, SessionSnapshot } from './types';

const isTauri = '__TAURI_INTERNALS__' in window;

async function call<T>(command: string, fallback: T): Promise<T> {
  if (!isTauri) {
    await new Promise((resolve) => setTimeout(resolve, 180));
    return fallback;
  }

  try {
    return await invoke<T>(command);
  } catch (error) {
    console.warn(`PaneLink command failed: ${command}`, error);
    return fallback;
  }
}

export function getCapabilities() {
  return call<Capabilities>('get_capabilities', fallbackCapabilities);
}

export function listPeers() {
  return call<Peer[]>('list_peers', fallbackPeers);
}

export function getSession() {
  return call<SessionSnapshot>('get_session_snapshot', fallbackSession);
}

export function listAudioDevices() {
  return call<AudioDevice[]>('list_audio_devices', fallbackDevices);
}

export function getPermissions() {
  return call<PermissionState[]>('get_permissions', fallbackPermissions);
}

export function connectPeer(peerId: string) {
  if (!isTauri) {
    return new Promise<SessionSnapshot>((resolve) =>
      setTimeout(() => resolve({ ...fallbackSession, status: 'connected', activePeerId: peerId }), 350),
    );
  }

  return invoke<SessionSnapshot>('connect_peer', { peerId });
}

export function disconnectPeer() {
  if (!isTauri) {
    return new Promise<SessionSnapshot>((resolve) =>
      setTimeout(() => resolve({ ...fallbackSession, status: 'ready', activePeerId: null }), 180),
    );
  }

  return invoke<SessionSnapshot>('disconnect_peer');
}
