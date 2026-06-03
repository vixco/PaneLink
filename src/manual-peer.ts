import type { Peer } from './types';

export function createManualPeer(input: string): Peer {
  const host = normalizeManualHost(input);

  return {
    id: `manual:${host}`,
    name: `Manual Mac ${host}`,
    os: 'macOS',
    address: host.includes(':') ? host : `${host}:48170`,
    lastSeen: 'Manual',
    status: 'online',
    trusted: true,
    latencyMs: 0,
  };
}

function normalizeManualHost(input: string) {
  const trimmed = input.trim();
  if (!trimmed) {
    return '';
  }

  try {
    const url = new URL(trimmed.includes('://') ? trimmed : `http://${trimmed}`);
    return url.port ? `${url.hostname}:${url.port}` : url.hostname;
  } catch {
    return trimmed.replace(/^https?:\/\//, '').split('/')[0] ?? trimmed;
  }
}
