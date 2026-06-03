import type { Capabilities, VideoBackendReport } from './types';

export type DisplayPipeline =
  | { kind: 'native-video' }
  | { kind: 'frame-fallback' }
  | { kind: 'unavailable' };

type VideoBackendInput = Pick<VideoBackendReport, 'canStartSourceStream'> | null;
type CapabilitiesInput = Pick<Capabilities, 'display'> | null;

export function selectDisplayPipeline(
  videoBackend: VideoBackendInput,
  capabilities: CapabilitiesInput,
): DisplayPipeline {
  if (videoBackend?.canStartSourceStream) {
    return { kind: 'native-video' };
  }

  if (capabilities?.display.capture === 'available' || capabilities?.display.capture === 'permission-required') {
    return { kind: 'frame-fallback' };
  }

  return { kind: 'unavailable' };
}
