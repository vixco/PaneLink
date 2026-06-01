import {
  Activity,
  AudioLines,
  BadgeCheck,
  Cable,
  Check,
  ChevronDown,
  CircleDot,
  Cpu,
  Gauge,
  Headphones,
  Keyboard,
  Monitor,
  MousePointer2,
  Plus,
  Radio,
  RotateCcw,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Trash2,
  Volume2,
  Wifi,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import {
  connectPeer,
  disconnectPeer,
  addRemoteScreen,
  getCapabilities,
  getPermissions,
  getSession,
  getStreamState,
  listAudioDevices,
  listPeers,
  removeRemoteScreen,
  startStream,
  stopStream,
} from './tauri';
import type { AudioDevice, Capabilities, Peer, PermissionState, RemoteScreen, SessionSnapshot, StreamState } from './types';
import { checkAndInstallUpdate, type UpdateStatus } from './updater';

type AppData = {
  peers: Peer[];
  session: SessionSnapshot | null;
  capabilities: Capabilities | null;
  audioDevices: AudioDevice[];
  permissions: PermissionState[];
  stream: StreamState | null;
};

const navItems = [
  { label: 'Connection', icon: Cable },
  { label: 'Displays', icon: Monitor },
  { label: 'Audio', icon: Headphones },
  { label: 'Input', icon: Keyboard },
  { label: 'Settings', icon: Settings },
];

const qualityOptions = ['Low latency', 'Balanced', 'Sharp'] as const;

function App() {
  const [data, setData] = useState<AppData>({
    peers: [],
    session: null,
    capabilities: null,
    audioDevices: [],
    permissions: [],
    stream: null,
  });
  const [selectedPeerId, setSelectedPeerId] = useState('windows-desk');
  const [quality, setQuality] = useState<StreamState['quality']>('Low latency');
  const [screens, setScreens] = useState<RemoteScreen[]>([]);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: 'idle', label: 'Updates ready' });
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    Promise.all([listPeers(), getSession(), getCapabilities(), listAudioDevices(), getPermissions(), getStreamState()]).then(
      ([peers, session, capabilities, audioDevices, permissions, stream]) => {
        setData({ peers, session, capabilities, audioDevices, permissions, stream });
        setSelectedPeerId(session.activePeerId ?? peers[0]?.id ?? '');
        setScreens(session.screens);
      },
    );
  }, []);

  useEffect(() => {
    void checkAndInstallUpdate(setUpdateStatus);
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      Promise.all([getSession(), getStreamState(), listPeers()]).then(([session, stream, peers]) => {
        setData((current) => ({ ...current, session, stream, peers }));
        setScreens(session.screens);
      });
    }, 1200);

    return () => window.clearInterval(timer);
  }, []);

  const selectedPeer = useMemo(
    () => data.peers.find((peer) => peer.id === selectedPeerId) ?? data.peers[0],
    [data.peers, selectedPeerId],
  );
  const session = data.session;
  const isConnected = session?.status === 'connected' || session?.status === 'degraded';
  const stream = data.stream;
  const isStreaming = stream?.status === 'streaming' || stream?.status === 'live';

  async function handleSwitch() {
    if (!selectedPeer) return;
    setIsBusy(true);
    try {
      if (isConnected) {
        const nextStream = await stopStream();
        const next = await disconnectPeer();
        setData((current) => ({ ...current, session: next, stream: nextStream }));
        setScreens(next.screens);
        return;
      }

      const next = await connectPeer(selectedPeer.id);
      const nextStream = await startStream({
        peerId: selectedPeer.id,
        screenIds: next.screens.map((screen) => screen.id),
        quality,
      });
      setData((current) => ({ ...current, session: next, stream: nextStream }));
      setScreens(next.screens);
    } finally {
      setIsBusy(false);
    }
  }

  async function addScreen() {
    if (!selectedPeer || screens.length >= 3) return;
    setIsBusy(true);
    try {
      const next = await addRemoteScreen(selectedPeer.id);
      setData((current) => ({ ...current, session: next }));
      setScreens(next.screens);

      if (isConnected) {
        const nextStream = await startStream({
          peerId: selectedPeer.id,
          screenIds: next.screens.map((screen) => screen.id),
          quality,
        });
        setData((current) => ({ ...current, stream: nextStream }));
      }
    } finally {
      setIsBusy(false);
    }
  }

  async function removeScreen(id: string) {
    if (screens.length === 1) return;
    setIsBusy(true);
    try {
      const next = await removeRemoteScreen(id);
      setData((current) => ({ ...current, session: next }));
      setScreens(next.screens);

      if (isConnected && selectedPeer) {
        const nextStream = await startStream({
          peerId: selectedPeer.id,
          screenIds: next.screens.map((screen) => screen.id),
          quality,
        });
        setData((current) => ({ ...current, stream: nextStream }));
      }
    } finally {
      setIsBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">PL</div>
          <div>
            <strong>PaneLink</strong>
            <span>Instant local display switching</span>
          </div>
        </div>
        <nav>
          {navItems.map((item, index) => (
            <button className={index === 0 ? 'nav-item active' : 'nav-item'} key={item.label}>
              <item.icon size={17} />
              {item.label}
            </button>
          ))}
        </nav>
        <div className={`update-card ${updateStatus.state}`}>
          <strong>{updateStatus.label}</strong>
          <span>Automatic releases via GitHub.</span>
        </div>
        <div className="sidebar-card">
          <ShieldCheck size={18} />
          <div>
            <strong>Trusted LAN only</strong>
            <span>Encrypted direct sessions with pairing tokens.</span>
          </div>
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <h1>Switch workspace</h1>
            <p>
              Connect a MacBook and Windows desk over the local network with low-latency video, audio and input
              channels.
            </p>
          </div>
          <div className={`status-pill ${isConnected ? 'connected' : 'ready'}`}>
            <CircleDot size={13} />
            {isConnected ? 'Connected' : isBusy ? 'Connecting' : 'Ready'}
          </div>
        </header>

        <div className="content-grid">
          <section className="panel connection-panel">
            <div className="panel-header">
              <div>
                <span className="section-label">Peer</span>
                <h2>Devices</h2>
              </div>
              <Wifi size={18} />
            </div>
            <div className="device-list">
              {data.peers.map((peer) => (
                <button
                  className={peer.id === selectedPeerId ? 'device-row selected' : 'device-row'}
                  key={peer.id}
                  onClick={() => setSelectedPeerId(peer.id)}
                >
                  <Monitor size={19} />
                  <span>
                    <strong>{peer.name}</strong>
                    <small>
                      {peer.os} - {peer.address}
                    </small>
                  </span>
                  <em>{peer.latencyMs} ms</em>
                </button>
              ))}
            </div>
            <button className="primary-action" disabled={isBusy || !selectedPeer} onClick={handleSwitch}>
              {isConnected ? 'Disconnect' : 'Switch Display'}
            </button>
            <div className="pairing-strip">
              <BadgeCheck size={16} />
              Pairing ready - mDNS discovery - QUIC LAN path
            </div>
          </section>

          <section className="stage-panel">
            <div className="device-bridge">
              <DeviceEndpoint title="This MacBook" subtitle="Capture + audio source" os="macOS" active />
              <div className="link-rail">
                <span />
                <strong>{session?.latencyMs ?? 0} ms</strong>
                <span />
              </div>
              <DeviceEndpoint
                title={selectedPeer?.name ?? 'Windows Desk'}
                subtitle="Receiver + display host"
                os={selectedPeer?.os ?? 'Windows'}
              />
            </div>

            <div className="screen-toolbar">
              <div>
                <span className="section-label">Screens</span>
                <h2>
                  {screens.length} remote display{screens.length > 1 ? 's' : ''}
                </h2>
              </div>
              <button className="secondary-action" disabled={isBusy || screens.length >= 3} onClick={addScreen}>
                <Plus size={15} />
                Add screen
              </button>
            </div>

            <div className="screen-slots">
              {screens.map((screen, index) => (
                <div className="screen-slot" key={screen.id}>
                  <span>{index + 1}</span>
                  <strong>{screen.targetDisplay}</strong>
                  <small>
                    {screen.fittedResolution} - {screen.scaleMode}
                  </small>
                  <em className={screen.status}>{screen.status}</em>
                  <button
                    aria-label={`Remove ${screen.targetDisplay}`}
                    disabled={screens.length === 1}
                    onClick={() => removeScreen(screen.id)}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              ))}
            </div>

            <div className="display-preview">
              <div className="preview-toolbar">
                <span>{session?.display ?? 'Display preview'}</span>
                <div>
                  <button>{stream?.quality ?? quality}</button>
                  <button>{isStreaming ? 'Live' : 'Standby'}</button>
                </div>
              </div>
              <div className="preview-frame">
                <div className={`screen-grid screen-count-${screens.length}`}>
                  {screens.map((screen, index) => (
                    <div key={screen.id}>
                      <span>Screen {index + 1}</span>
                      <small>{screen.fittedResolution.replace(' @ ', ' fit ')}</small>
                    </div>
                  ))}
                </div>
                <div className="preview-copy">
                  <Monitor size={28} />
                  <strong>{isStreaming ? 'Live display stream active' : 'Ready to start direct display session'}</strong>
                  <span>
                    {stream && stream.width > 0
                      ? `${stream.width} x ${stream.height} - frame ${stream.frameId.toLocaleString()}`
                      : (session?.resolution ?? 'No resolution negotiated yet')}{' '}
                    - auto-fit and rollback enabled
                  </span>
                </div>
              </div>
            </div>
            <div className="stream-strip">
              <span className={isStreaming ? 'stream-dot live' : 'stream-dot'} />
              <strong>{stream?.status ?? 'idle'}</strong>
              <span>{stream?.codec ?? session?.encoder ?? 'Encoder ready'}</span>
              <span>{stream?.fps ?? session?.fps ?? 0} FPS</span>
              <span>{stream?.latencyMs ?? session?.latencyMs ?? 0} ms</span>
              <span>{stream?.jitterMs ?? 0} ms jitter</span>
            </div>
            <div className="rollback-strip">
              <RotateCcw size={15} />
              Previous display layout is saved before connect and restored automatically on disconnect.
            </div>
          </section>

          <aside className="right-stack">
            <MetricPanel session={session} stream={stream} />
            <AudioPanel devices={data.audioDevices} />
            <PermissionsPanel permissions={data.permissions} capabilities={data.capabilities} />
          </aside>
        </div>

        <footer className="statusbar">
          <span>
            <Radio size={14} /> {session?.transport ?? 'LAN QUIC'}
          </span>
          <span>
            <Cpu size={14} /> {session?.encoder ?? 'H.264 low latency'}
          </span>
          <span>
            <Activity size={14} /> {stream?.fps ?? session?.fps ?? 0} FPS
          </span>
          <span>
            <Gauge size={14} /> {stream?.bitrateMbps ?? session?.bitrateMbps ?? 0} Mbps
          </span>
          <div className="quality-control">
            {qualityOptions.map((option) => (
              <button
                className={quality === option ? 'active' : ''}
                key={option}
                onClick={() => {
                  setQuality(option);
                  if (isConnected && selectedPeer) {
                    void startStream({
                      peerId: selectedPeer.id,
                      screenIds: screens.map((screen) => screen.id),
                      quality: option,
                    }).then((nextStream) => setData((current) => ({ ...current, stream: nextStream })));
                  }
                }}
              >
                {option}
              </button>
            ))}
          </div>
        </footer>
      </section>
    </main>
  );
}

function DeviceEndpoint({ title, subtitle, os, active = false }: { title: string; subtitle: string; os: string; active?: boolean }) {
  return (
    <div className={active ? 'endpoint active' : 'endpoint'}>
      <Monitor size={24} />
      <strong>{title}</strong>
      <span>{subtitle}</span>
      <em>{os}</em>
    </div>
  );
}

function MetricPanel({ session, stream }: { session: SessionSnapshot | null; stream: StreamState | null }) {
  return (
    <section className="panel compact">
      <div className="panel-header">
        <h2>Performance</h2>
        <SlidersHorizontal size={17} />
      </div>
      <div className="metrics">
        <Metric label="Latency" value={`${stream?.latencyMs ?? session?.latencyMs ?? 0} ms`} good />
        <Metric label="Frame rate" value={`${stream?.fps ?? session?.fps ?? 0} FPS`} />
        <Metric label="Bitrate" value={`${stream?.bitrateMbps ?? session?.bitrateMbps ?? 0} Mbps`} />
        <Metric label="Loss" value={`${stream?.packetLoss ?? session?.packetLoss ?? 0}%`} good />
      </div>
      <div className="stream-card">
        <span>Stream</span>
        <strong>{stream?.status ?? 'idle'}</strong>
        <small>
          {stream?.screenIds.length ?? 0} screen{stream?.screenIds.length === 1 ? '' : 's'} - audio{' '}
          {stream?.audioActive ? 'on' : 'ready'} - mic {stream?.microphoneActive ? 'on' : 'ready'}
        </small>
      </div>
    </section>
  );
}

function Metric({ label, value, good = false }: { label: string; value: string; good?: boolean }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong className={good ? 'good' : ''}>{value}</strong>
    </div>
  );
}

function AudioPanel({ devices }: { devices: AudioDevice[] }) {
  const outputs = devices.filter((device) => device.kind === 'output');
  const inputs = devices.filter((device) => device.kind === 'input');

  return (
    <section className="panel compact">
      <div className="panel-header">
        <h2>Audio routes</h2>
        <AudioLines size={17} />
      </div>
      <DeviceSelect icon={<Volume2 size={16} />} label="Output" devices={outputs} />
      <DeviceSelect icon={<MousePointer2 size={16} />} label="Microphone" devices={inputs} />
      <div className="meter">
        <span />
        <span />
        <span />
        <span />
        <span />
      </div>
    </section>
  );
}

function DeviceSelect({ icon, label, devices }: { icon: React.ReactNode; label: string; devices: AudioDevice[] }) {
  const device = devices.find((item) => item.isDefault) ?? devices[0];
  return (
    <button className="select-row">
      {icon}
      <span>
        <small>{label}</small>
        <strong>{device?.name ?? 'No device'}</strong>
      </span>
      <ChevronDown size={15} />
    </button>
  );
}

function PermissionsPanel({ permissions, capabilities }: { permissions: PermissionState[]; capabilities: Capabilities | null }) {
  return (
    <section className="panel compact">
      <div className="panel-header">
        <h2>System readiness</h2>
        <Check size={17} />
      </div>
      <div className="permission-list">
        {permissions.map((permission) => (
          <div className="permission-row" key={permission.key}>
            <span className={permission.status}>{permission.status}</span>
            <strong>{permission.label}</strong>
            <small>{permission.detail}</small>
          </div>
        ))}
      </div>
      <p className="capability-note">
        Virtual display: {capabilities?.display.virtualDisplay ?? 'checking'} - audio routing:{' '}
        {capabilities?.audio.virtualRouting ?? 'checking'}
      </p>
    </section>
  );
}

export default App;
