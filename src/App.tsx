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
  Plus,
  MousePointer2,
  Radio,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Volume2,
  Wifi,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import {
  connectPeer,
  disconnectPeer,
  getCapabilities,
  getPermissions,
  getSession,
  listAudioDevices,
  listPeers,
} from './tauri';
import type { AudioDevice, Capabilities, Peer, PermissionState, SessionSnapshot } from './types';

type AppData = {
  peers: Peer[];
  session: SessionSnapshot | null;
  capabilities: Capabilities | null;
  audioDevices: AudioDevice[];
  permissions: PermissionState[];
};

const navItems = [
  { label: 'Connection', icon: Cable },
  { label: 'Displays', icon: Monitor },
  { label: 'Audio', icon: Headphones },
  { label: 'Input', icon: Keyboard },
  { label: 'Settings', icon: Settings },
];

function App() {
  const [data, setData] = useState<AppData>({
    peers: [],
    session: null,
    capabilities: null,
    audioDevices: [],
    permissions: [],
  });
  const [selectedPeerId, setSelectedPeerId] = useState('windows-desk');
  const [quality, setQuality] = useState('Low latency');
  const [screenCount, setScreenCount] = useState(1);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    Promise.all([listPeers(), getSession(), getCapabilities(), listAudioDevices(), getPermissions()]).then(
      ([peers, session, capabilities, audioDevices, permissions]) => {
        setData({ peers, session, capabilities, audioDevices, permissions });
        setSelectedPeerId(session.activePeerId ?? peers[0]?.id ?? '');
      },
    );
  }, []);

  const selectedPeer = useMemo(
    () => data.peers.find((peer) => peer.id === selectedPeerId) ?? data.peers[0],
    [data.peers, selectedPeerId],
  );
  const session = data.session;
  const isConnected = session?.status === 'connected' || session?.status === 'degraded';

  async function handleSwitch() {
    if (!selectedPeer) return;
    setIsBusy(true);
    const next = isConnected ? await disconnectPeer() : await connectPeer(selectedPeer.id);
    setData((current) => ({ ...current, session: next }));
    setIsBusy(false);
  }

  function addScreen() {
    setScreenCount((count) => Math.min(count + 1, 3));
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
                      {peer.os} · {peer.address}
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
              Pairing ready · mDNS discovery · QUIC LAN path
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
                <h2>{screenCount} remote display{screenCount > 1 ? 's' : ''}</h2>
              </div>
              <button className="secondary-action" disabled={screenCount >= 3} onClick={addScreen}>
                <Plus size={15} />
                Add screen
              </button>
            </div>

            <div className="display-preview">
              <div className="preview-toolbar">
                <span>{session?.display ?? 'Display preview'}</span>
                <div>
                  <button>Fit</button>
                  <button>Fullscreen</button>
                </div>
              </div>
              <div className="preview-frame">
                <div className={`screen-grid screen-count-${screenCount}`}>
                  {Array.from({ length: screenCount }).map((_, index) => (
                    <div key={index}>
                      <span>Screen {index + 1}</span>
                      <small>{index === 0 ? '2560 x 1440' : index === 1 ? '1920 x 1080 fit' : '1440 x 900 fit'}</small>
                    </div>
                  ))}
                </div>
                <div className="preview-copy">
                  <Monitor size={28} />
                  <strong>{isConnected ? 'Live display stream active' : 'Ready to start direct display session'}</strong>
                  <span>
                    {session?.resolution ?? 'No resolution negotiated yet'} · auto-fit and rollback enabled
                  </span>
                </div>
              </div>
            </div>
          </section>

          <aside className="right-stack">
            <MetricPanel session={session} />
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
            <Activity size={14} /> {session?.fps ?? 0} FPS
          </span>
          <span>
            <Gauge size={14} /> {session?.bitrateMbps ?? 0} Mbps
          </span>
          <div className="quality-control">
            {['Low latency', 'Balanced', 'Sharp'].map((option) => (
              <button className={quality === option ? 'active' : ''} key={option} onClick={() => setQuality(option)}>
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

function MetricPanel({ session }: { session: SessionSnapshot | null }) {
  return (
    <section className="panel compact">
      <div className="panel-header">
        <h2>Performance</h2>
        <SlidersHorizontal size={17} />
      </div>
      <div className="metrics">
        <Metric label="Latency" value={`${session?.latencyMs ?? 0} ms`} good />
        <Metric label="Frame rate" value={`${session?.fps ?? 0} FPS`} />
        <Metric label="Bitrate" value={`${session?.bitrateMbps ?? 0} Mbps`} />
        <Metric label="Loss" value={`${session?.packetLoss ?? 0}%`} good />
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
        Virtual display: {capabilities?.display.virtualDisplay ?? 'checking'} · audio routing:{' '}
        {capabilities?.audio.virtualRouting ?? 'checking'}
      </p>
    </section>
  );
}

export default App;
