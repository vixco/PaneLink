export type ConnectionStatus = 'ready' | 'connecting' | 'connected' | 'degraded' | 'offline';
export type StreamStatus = 'idle' | 'starting' | 'streaming' | 'live' | 'stopping' | 'error';

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
  displayPlan: DisplaySessionPlan | null;
  rollbackSnapshot: RollbackSnapshot | null;
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

export type StreamState = {
  status: StreamStatus;
  activePeerId: string | null;
  screenIds: string[];
  codec: string;
  transport: 'LAN QUIC' | 'WebRTC' | 'Local preview';
  quality: 'Low latency' | 'Balanced' | 'Sharp';
  width: number;
  height: number;
  targetFps: number;
  fps: number;
  bitrateMbps: number;
  latencyMs: number;
  jitterMs: number;
  packetLoss: number;
  frameId: number;
  audioActive: boolean;
  microphoneActive: boolean;
  updatedAt: string;
  error: string | null;
};

export type StartStreamRequest = {
  peerId: string;
  screenIds: string[];
  quality: StreamState['quality'];
};

export type DisplayWindowRequest = {
  peerId: string;
  screenCount: number;
  quality: StreamState['quality'];
};

export type DisplayWindowState = {
  attached: boolean;
  message: string;
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

export type DisplaySessionPlan = {
  id: string;
  peerId: string;
  windowsPc: DisplayTopology;
  screens: PlannedScreen[];
  rollbackSnapshot: RollbackSnapshot;
};

export type DisplayTopology = {
  pcId: string;
  pcName: string;
  displays: TargetDisplay[];
};

export type SourceDisplay = {
  id: string;
  name: string;
  nativeMode: DisplayMode;
  currentMode: DisplayMode;
};

export type TargetDisplay = {
  id: string;
  name: string;
  role: 'primary' | 'extended';
  nativeMode: DisplayMode;
  currentMode: DisplayMode;
  supportedModes: DisplayMode[];
  bounds: DisplayRect;
  attached: boolean;
};

export type DisplayMode = {
  width: number;
  height: number;
  refreshHz: number;
};

export type DisplayRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type PlannedScreen = {
  sourceDisplay: SourceDisplay;
  targetDisplay: TargetDisplay;
  selectedMode: DisplayMode;
  fittedResolution: FittedResolution;
  remoteScreen: RemoteScreen;
};

export type FittedResolution = {
  width: number;
  height: number;
  refreshHz: number;
  insets: ScaleInsets;
};

export type ScaleInsets = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

export type RollbackSnapshot = {
  id: string;
  peerId: string;
  reason: 'disconnect' | 'crash-recovery' | 'user-cancel';
  localLayout: DisplayLayout;
  remoteLayout: DisplayLayout;
};

export type DisplayLayout = {
  pcId: string;
  displays: DisplayLayoutEntry[];
};

export type DisplayLayoutEntry = {
  displayId: string;
  mode: DisplayMode;
  bounds: DisplayRect;
  primary: boolean;
  attached: boolean;
};
