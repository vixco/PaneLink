export type ConnectionStatus = 'ready' | 'connecting' | 'connected' | 'degraded' | 'offline';

export type Peer = {
  id: string;
  name: string;
  os: 'macOS' | 'Windows';
  address: string;
  lastSeen: string;
  status: 'online' | 'sleeping' | 'offline' | 'pairing';
  trusted: boolean;
  latencyMs: number;
};

export type Capabilities = {
  appVersion: string;
  peerId: string;
  platform: string;
  videoEncoders: string[];
  transport: string[];
  audio: {
    outputCapture: boolean;
    microphoneCapture: boolean;
    virtualRouting: 'planned' | 'available' | 'unavailable';
  };
  display: {
    capture: 'available' | 'permission-required' | 'stub';
    virtualDisplay: 'driver-required' | 'available' | 'planned';
  };
};

export type AudioDevice = {
  id: string;
  name: string;
  kind: 'output' | 'input';
  isDefault: boolean;
  available: boolean;
};

export type PermissionState = {
  key: string;
  label: string;
  status: 'granted' | 'required' | 'unsupported';
  detail: string;
};

export type SessionSnapshot = {
  status: ConnectionStatus;
  activePeerId: string | null;
  display: string;
  resolution: string;
  screens: RemoteScreen[];
  fps: number;
  latencyMs: number;
  bitrateMbps: number;
  packetLoss: number;
  encoder: string;
  transport: 'LAN QUIC' | 'WebRTC' | 'Local preview';
  audioOutput: string;
  micInput: string;
};

export type RemoteScreen = {
  id: string;
  name: string;
  role: 'primary' | 'extended';
  sourceDisplay: string;
  targetDisplay: string;
  nativeResolution: string;
  fittedResolution: string;
  scaleMode: 'auto-fit' | 'native' | 'manual';
  status: 'ready' | 'connected' | 'rollback-pending';
};
