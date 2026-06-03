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
  peerAddress: string;
  controlAddress: string;
  videoSessionId?: string;
  videoTransport?: string;
  videoCodec?: string;
  screenCount: number;
  quality: StreamState['quality'];
};

export type DisplayWindowState = {
  attached: boolean;
  message: string;
};

export type RemoteDisplayResponse = {
  ok: boolean;
  message: string;
};

export type RemoteFrameResponse = {
  ok: boolean;
  statusCode: number;
  contentType: string;
  dataUrl: string | null;
  message: string;
};

export type NativeSetupState = {
  started: boolean;
  platform: string;
  message: string;
  actions: string[];
  requiresRestart: boolean;
};

export type VirtualDisplayBackendReport = {
  backend: string;
  state: 'available' | 'driver-required' | 'unsupported';
  available: boolean;
  requiresExternalTool: boolean;
  message: string;
  actions: string[];
};

export type VirtualDisplayRequest = {
  name: string;
  width: number;
  height: number;
  refreshHz: number;
};

export type VirtualDisplaySession = {
  id: string;
  active: boolean;
  backend: string;
  displayName: string;
  platformDisplayId?: number | null;
  width: number;
  height: number;
  refreshHz: number;
  message: string;
};

export type VideoBackendReport = {
  backend: string;
  state: 'available' | 'permission-required' | 'receiver-only' | 'unsupported';
  available: boolean;
  canStartSourceStream: boolean;
  transport: string;
  codec: string;
  hardwareAccelerated: boolean;
  message: string;
  actions: string[];
};

export type VideoSessionRequest = {
  sourcePeerId: string;
  receiverPeerId: string;
  screenCount: number;
  quality: StreamState['quality'];
  width: number;
  height: number;
  refreshHz: number;
  controlAddress: string;
};

export type VideoSession = {
  id: string;
  active: boolean;
  endpoint: string;
  controlAddress: string;
  transport: string;
  codec: string;
  quality: StreamState['quality'];
  targetFps: number;
  targetBitrateMbps: number;
  screenCount: number;
  width: number;
  height: number;
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
