import type {
  AudioDevice,
  Capabilities,
  Peer,
  PermissionState,
  SessionSnapshot,
  StreamState,
  VideoBackendReport,
  VirtualDisplayBackendReport,
} from './types';

export const fallbackPeers: Peer[] = [
  {
    id: 'macbook-pro',
    name: 'This MacBook',
    os: 'macOS',
    address: '192.168.1.24',
    lastSeen: 'Now',
    status: 'online',
    trusted: false,
    latencyMs: 7,
  },
  {
    id: 'windows-desk',
    name: 'Windows Desk',
    os: 'Windows',
    address: '192.168.1.42',
    lastSeen: 'Now',
    status: 'online',
    trusted: false,
    latencyMs: 9,
  },
];

export const fallbackCapabilities: Capabilities = {
  appVersion: '0.1.0',
  peerId: 'dev-preview',
  platform: 'browser-preview',
  videoEncoders: ['H.264 low latency', 'HEVC planned', 'AV1 planned'],
  transport: ['LAN QUIC', 'mDNS discovery', 'WebRTC planned'],
  audio: {
    outputCapture: true,
    microphoneCapture: true,
    virtualRouting: 'planned',
  },
  display: {
    capture: 'stub',
    virtualDisplay: 'driver-required',
  },
};

export const fallbackDevices: AudioDevice[] = [
  { id: 'default-output', name: 'System Default Output', kind: 'output', isDefault: true, available: true },
  { id: 'headset-output', name: 'Wireless Headset', kind: 'output', isDefault: false, available: true },
  { id: 'default-input', name: 'System Default Microphone', kind: 'input', isDefault: true, available: true },
  { id: 'headset-input', name: 'Wireless Headset Mic', kind: 'input', isDefault: false, available: true },
];

export const fallbackPermissions: PermissionState[] = [
  {
    key: 'screen-capture',
    label: 'Screen capture',
    status: 'required',
    detail: 'Required on macOS; Windows uses DXGI Desktop Duplication.',
  },
  {
    key: 'accessibility',
    label: 'Input control',
    status: 'required',
    detail: 'Needed to forward keyboard and pointer events on macOS.',
  },
  {
    key: 'audio-routing',
    label: 'Virtual audio routing',
    status: 'unsupported',
    detail: 'Requires signed virtual audio drivers for full system routing.',
  },
];

export const fallbackVirtualDisplayBackend: VirtualDisplayBackendReport = {
  backend: 'Browser preview',
  state: 'driver-required',
  available: false,
  requiresExternalTool: false,
  message: 'Virtual displays are only available in the installed macOS PaneLink app.',
  actions: [],
};

export const fallbackVideoBackend: VideoBackendReport = {
  backend: 'Browser preview receiver',
  state: 'receiver-only',
  available: true,
  canStartSourceStream: false,
  transport: 'WebRTC/RTP preview',
  codec: 'H.264 browser decode',
  hardwareAccelerated: true,
  message: 'Native remote-desktop video is available in the installed PaneLink app; browser preview simulates the session contract.',
  actions: [],
};

export const fallbackSession: SessionSnapshot = {
  status: 'ready',
  activePeerId: 'windows-desk',
  display: 'Desk monitor',
  resolution: '2560 x 1440 @ 120 Hz',
  displayPlan: null,
  rollbackSnapshot: null,
  screens: [
    {
      id: 'screen-main',
      name: 'Desk monitor',
      role: 'primary',
      sourceDisplay: 'MacBook display',
      targetDisplay: 'Windows Display 1',
      nativeResolution: '2560 x 1440 @ 120 Hz',
      fittedResolution: '2560 x 1440 @ 120 Hz',
      scaleMode: 'auto-fit',
      status: 'ready',
    },
  ],
  fps: 120,
  latencyMs: 9,
  bitrateMbps: 58,
  packetLoss: 0.1,
  encoder: 'H.264 VideoToolbox',
  transport: 'WebRTC',
  audioOutput: 'System Default Output',
  micInput: 'System Default Microphone',
};

export const fallbackStreamState: StreamState = {
  status: 'idle',
  activePeerId: null,
  screenIds: [],
  codec: 'H.264 VideoToolbox',
  transport: 'WebRTC',
  quality: 'Low latency',
  width: 0,
  height: 0,
  targetFps: 120,
  fps: 0,
  bitrateMbps: 0,
  latencyMs: 0,
  jitterMs: 0,
  packetLoss: 0,
  frameId: 0,
  audioActive: true,
  microphoneActive: true,
  updatedAt: new Date().toISOString(),
  error: null,
};
