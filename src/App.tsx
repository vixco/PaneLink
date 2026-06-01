import {
  Activity,
  CheckCircle2,
  Loader2,
  Monitor,
  Plus,
  RefreshCw,
  Settings,
  Trash2,
  Volume2,
  Wifi,
  WifiOff,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import {
  addRemoteScreen,
  connectPeer,
  disconnectPeer,
  getCapabilities,
  getPermissions,
  getSession,
  getStreamState,
  listAudioDevices,
  listPeers,
  removeRemoteScreen,
  scanPeers,
  startStream,
  stopStream,
} from './tauri';
import type { AudioDevice, Capabilities, Peer, PermissionState, RemoteScreen, SessionSnapshot, StreamState } from './types';
import { checkAndInstallUpdate, type UpdateStatus } from './updater';

type AppData = {
  peers: Peer[];
  session: SessionSnapshot | null;
  stream: StreamState | null;
  capabilities: Capabilities | null;
  audioDevices: AudioDevice[];
  permissions: PermissionState[];
};

const qualities: StreamState['quality'][] = ['Low latency', 'Balanced', 'Sharp'];

function App() {
  const [data, setData] = useState<AppData>({
    peers: [],
    session: null,
    stream: null,
    capabilities: null,
    audioDevices: [],
    permissions: [],
  });
  const [selectedPeerId, setSelectedPeerId] = useState('');
  const [quality, setQuality] = useState<StreamState['quality']>('Low latency');
  const [isBusy, setIsBusy] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: 'idle', label: 'Updates ready' });

  useEffect(() => {
    void loadEverything(true);
    void checkAndInstallUpdate(setUpdateStatus);
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshLiveState();
    }, 1800);

    return () => window.clearInterval(timer);
  }, []);

  const session = data.session;
  const stream = data.stream;
  const selectedPeer = useMemo(
    () => data.peers.find((peer) => peer.id === selectedPeerId) ?? data.peers[0],
    [data.peers, selectedPeerId],
  );
  const screens = session?.screens ?? [];
  const isConnected = session?.status === 'connected' || session?.status === 'degraded';
  const isStreaming = stream?.status === 'streaming' || stream?.status === 'live';

  async function loadEverything(scan = false) {
    setIsScanning(scan);
    try {
      const [peers, session, stream, capabilities, audioDevices, permissions] = await Promise.all([
        scan ? scanPeers() : listPeers(),
        getSession(),
        getStreamState(),
        getCapabilities(),
        listAudioDevices(),
        getPermissions(),
      ]);

      setData({ peers, session, stream, capabilities, audioDevices, permissions });
      setSelectedPeerId((current) => session.activePeerId ?? (current || peers[0]?.id || ''));
    } finally {
      setIsScanning(false);
    }
  }

  async function refreshLiveState() {
    const [peers, session, stream] = await Promise.all([listPeers(), getSession(), getStreamState()]);
    setData((current) => ({ ...current, peers, session, stream }));
    setSelectedPeerId((current) => session.activePeerId ?? (current || peers[0]?.id || ''));
  }

  async function handleConnect() {
    if (!selectedPeer) return;

    setIsBusy(true);
    try {
      if (isConnected) {
        const nextStream = await stopStream();
        const nextSession = await disconnectPeer();
        setData((current) => ({ ...current, session: nextSession, stream: nextStream }));
        return;
      }

      const nextSession = await connectPeer(selectedPeer.id);
      const nextStream = await startStream({
        peerId: selectedPeer.id,
        screenIds: nextSession.screens.map((screen) => screen.id),
        quality,
      });
      setData((current) => ({ ...current, session: nextSession, stream: nextStream }));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleAddScreen() {
    if (!selectedPeer || screens.length >= 3) return;

    setIsBusy(true);
    try {
      const nextSession = await addRemoteScreen(selectedPeer.id);
      const nextStream = isConnected
        ? await startStream({
            peerId: selectedPeer.id,
            screenIds: nextSession.screens.map((screen) => screen.id),
            quality,
          })
        : data.stream;
      setData((current) => ({ ...current, session: nextSession, stream: nextStream }));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleRemoveScreen(screenId: string) {
    if (!selectedPeer || screens.length <= 1) return;

    setIsBusy(true);
    try {
      const nextSession = await removeRemoteScreen(screenId);
      const nextStream = isConnected
        ? await startStream({
            peerId: selectedPeer.id,
            screenIds: nextSession.screens.map((screen) => screen.id),
            quality,
          })
        : data.stream;
      setData((current) => ({ ...current, session: nextSession, stream: nextStream }));
    } finally {
      setIsBusy(false);
    }
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <div className="brand">
          <div className="brand-mark">PL</div>
          <div>
            <strong>PaneLink</strong>
            <span>MacBook naar Windows monitor</span>
          </div>
        </div>
        <div className={isConnected ? 'status-pill connected' : 'status-pill'}>
          {isConnected ? <CheckCircle2 size={16} /> : <Wifi size={16} />}
          {isConnected ? 'Verbonden' : 'Klaar om te verbinden'}
        </div>
      </header>

      <section className="hero-panel">
        <div>
          <span className="section-label">Connections</span>
          <h1>Kies je andere computer</h1>
          <p>
            Zorg dat PaneLink op beide apparaten open staat. De app scant automatisch je lokale netwerk.
          </p>
        </div>
        <button className="scan-button" disabled={isScanning} onClick={() => loadEverything(true)}>
          {isScanning ? <Loader2 className="spin" size={17} /> : <RefreshCw size={17} />}
          Scan opnieuw
        </button>
      </section>

      <section className="connection-layout">
        <div className="peer-panel">
          <div className="panel-title">
            <Wifi size={18} />
            <h2>Gevonden apparaten</h2>
          </div>

          {data.peers.length === 0 ? (
            <div className="empty-state">
              <WifiOff size={28} />
              <strong>Geen MacBook of Windows pc gevonden</strong>
              <span>Open PaneLink op beide apparaten en laat Windows/macOS netwerktoegang toe.</span>
            </div>
          ) : (
            <div className="peer-list">
              {data.peers.map((peer) => (
                <button
                  className={peer.id === selectedPeer?.id ? 'peer-card selected' : 'peer-card'}
                  key={peer.id}
                  onClick={() => setSelectedPeerId(peer.id)}
                >
                  <Monitor size={22} />
                  <span>
                    <strong>{peer.name}</strong>
                    <small>
                      {peer.os} · {peer.address}
                    </small>
                  </span>
                  <em>{peer.status}</em>
                </button>
              ))}
            </div>
          )}

          <button className="primary-action" disabled={isBusy || !selectedPeer} onClick={handleConnect}>
            {isBusy ? <Loader2 className="spin" size={18} /> : <Monitor size={18} />}
            {isConnected ? 'Verbinding stoppen' : selectedPeer ? `Verbind met ${selectedPeer.name}` : 'Geen apparaat gevonden'}
          </button>
        </div>

        <div className="session-panel">
          <div className="session-top">
            <div>
              <span className="section-label">Session</span>
              <h2>{isConnected ? selectedPeer?.name ?? 'Verbonden apparaat' : 'Nog niet verbonden'}</h2>
            </div>
            <strong>{isStreaming ? `${stream?.fps ?? 0} FPS` : 'Standby'}</strong>
          </div>

          <div className="preview-frame">
            <div className={`screen-grid screen-count-${Math.max(screens.length, 1)}`}>
              {(screens.length ? screens : [{ id: 'placeholder', fittedResolution: 'Auto-fit ready' } as RemoteScreen]).map((screen, index) => (
                <div key={screen.id}>
                  <span>Screen {index + 1}</span>
                  <small>{screen.fittedResolution}</small>
                </div>
              ))}
            </div>
            <div className="preview-copy">
              <Monitor size={30} />
              <strong>{isStreaming ? 'Live stream actief' : 'Wacht op verbinding'}</strong>
              <span>{isConnected ? 'Auto-fit resolutie en audio staan klaar.' : 'Klik links op verbinden.'}</span>
            </div>
          </div>
        </div>
      </section>

      <section className="details-grid">
        <div className="detail-panel">
          <div className="panel-title">
            <Monitor size={18} />
            <h2>Schermen</h2>
          </div>
          <div className="screen-list">
            {screens.map((screen, index) => (
              <div className="screen-row" key={screen.id}>
                <span>{index + 1}</span>
                <strong>{screen.targetDisplay}</strong>
                <small>{screen.fittedResolution}</small>
                <button disabled={screens.length <= 1 || isBusy} onClick={() => handleRemoveScreen(screen.id)}>
                  <Trash2 size={15} />
                </button>
              </div>
            ))}
          </div>
          <button className="secondary-action" disabled={!isConnected || isBusy || screens.length >= 3} onClick={handleAddScreen}>
            <Plus size={16} />
            Add screen
          </button>
        </div>

        <div className="detail-panel">
          <div className="panel-title">
            <Activity size={18} />
            <h2>Kwaliteit</h2>
          </div>
          <div className="quality-control">
            {qualities.map((option) => (
              <button
                className={quality === option ? 'active' : ''}
                key={option}
                onClick={() => setQuality(option)}
              >
                {option}
              </button>
            ))}
          </div>
          <Metric label="Latency" value={`${stream?.latencyMs ?? session?.latencyMs ?? 0} ms`} />
          <Metric label="Bitrate" value={`${stream?.bitrateMbps ?? session?.bitrateMbps ?? 0} Mbps`} />
        </div>

        <div className="detail-panel">
          <div className="panel-title">
            <Volume2 size={18} />
            <h2>Audio</h2>
          </div>
          <AudioLine label="Output" devices={data.audioDevices.filter((device) => device.kind === 'output')} />
          <AudioLine label="Mic" devices={data.audioDevices.filter((device) => device.kind === 'input')} />
        </div>

        <div className="detail-panel">
          <div className="panel-title">
            <Settings size={18} />
            <h2>Status</h2>
          </div>
          <small className="muted">Update: {updateStatus.label}</small>
          <small className="muted">Peer ID: {data.capabilities?.peerId ?? 'laden...'}</small>
          {data.permissions.slice(0, 2).map((permission) => (
            <small className="muted" key={permission.key}>
              {permission.label}: {permission.status}
            </small>
          ))}
        </div>
      </section>
    </main>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function AudioLine({ label, devices }: { label: string; devices: AudioDevice[] }) {
  const device = devices.find((item) => item.isDefault) ?? devices[0];

  return (
    <div className="audio-line">
      <span>{label}</span>
      <strong>{device?.name ?? 'System default'}</strong>
    </div>
  );
}

export default App;
