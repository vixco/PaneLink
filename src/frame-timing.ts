import type { StreamState } from './types';

export function framePollDelayMs(quality: StreamState['quality'], frameLoaded: boolean) {
  if (!frameLoaded) {
    return 1000;
  }

  if (quality === 'Sharp') {
    return 16;
  }

  if (quality === 'Low latency') {
    return 33;
  }

  return 66;
}
