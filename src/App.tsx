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
  closeDisplayWindow,
  connectPeer,
  disconnectPeer,
  getCapabilities,
  getPermissions,
  getSession,
  getStreamState,
  listAudioDevices,
  listPeers,
  openDisplayWindow,
  removeRemoteScreen,
  runNativeSetup,
  scanPeers,
  startStream,
  stopStream,
} from './tauri';
import type {
  AudioDevice,
  Capabilities,
  DisplayWindowRequest,
  NativeSetupState,
  Peer,
  PermissionState,
  RemoteScreen,
  SessionSnapshot,
  StreamState,
} from './types';
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
const isTauriRuntime = '__TAURI_INTERNALS__' in window;
const queryIsDisplayWindow = new URLSearchParams(window.location.search).get('window') === 'display';

function App() {
  const [windowMode, setWindowMode] = useState<'loading' | 'control' | 'display'>(
    queryIsDisplayWindow ? 'display' : isTauriRuntime ? 'loading' : 'control',
  );

  useEffect(() => {
    if (queryIsDisplayWindow || !isTauriRuntime) return;

    let cancelled = false;

    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => {
        if (!cancelled) {
          setWindowMode(getCurrentWindow().label === 'display' ? 'display' : 'control');
        }
      })
      .catch(() => {
        if (!cancelled) {
          setWindowMode('control');
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  if (windowMode === 'loading') {
    return <DisplayBoot />;
  }

  if (windowMode === 'display') {
    return <DisplayWindow />;
  }

  return <ControlApp />;
}

function ControlApp() {
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
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [trustedPeerIds, setTrustedPeerIds] = useState<string[]>(() => loadTrustedPeerIds());
  const [pairingPeerId, setPairingPeerId] = useState('');
  const [pairingCode, setPairingCode] = useState('');
  const [pairingError, setPairingError] = useState('');
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: 'idle', label: 'Updates ready' });
  const [displayWindow, setDisplayWindow] = useState({ attached: false, message: 'Display window closed' });
  const [setupStatus, setSetupStatus] = useState<NativeSetupState | null>(null);

  useEffect(() => {
    void loadEverything(true);
    void handleCheckForUpdate();
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
  const displayPipelineReady = data.capabilities?.display.capture === 'available'
    || data.capabilities?.display.capture === 'permission-required';
  const receiverReady = isStreaming && displayWindow.attached;
  const selectedPeerTrusted = selectedPeer ? trustedPeerIds.includes(selectedPeer.id) || selectedPeer.trusted : false;
  const localPairingCode = data.capabilities?.peerId ? pairingCodeForPeer(data.capabilities.peerId) : 'laden...';

  async function startStreamAndOpenDisplay(peer: Peer, nextSession: SessionSnapshot) {
    const nextStream = await startStream({
      peerId: peer.id,
      screenIds: nextSession.screens.map((screen) => screen.id),
      quality,
    });
    const frameUrl = frameUrlForPeer(peer);

    if (!displayPipelineReady) {
      await closeDisplayWindow();
      const nextSetupStatus = await runNativeSetup();
      setSetupStatus(nextSetupStatus);
      setDisplayWindow({
        attached: false,
        message: nextSetupStatus.message,
      });

      return nextStream;
    }

    const nextDisplayWindow = await openDisplayWindow({
      peerId: peer.id,
      peerAddress: frameUrl,
      screenCount: nextSession.screens.length,
      quality,
    });
    setDisplayWindow(nextDisplayWindow);

    return nextStream;
  }

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
        const nextDisplayWindow = await closeDisplayWindow();
        setDisplayWindow(nextDisplayWindow);
        setData((current) => ({ ...current, session: nextSession, stream: nextStream }));
        return;
      }

      if (!selectedPeerTrusted) {
        setPairingPeerId(selectedPeer.id);
        setPairingCode('');
        setPairingError('');
        return;
      }

      const nextSession = await connectPeer(selectedPeer.id);
      const nextStream = await startStreamAndOpenDisplay(selectedPeer, nextSession);
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
      const nextStream = await startStreamAndOpenDisplay(selectedPeer, nextSession);
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
      const nextStream = selectedPeer ? await startStreamAndOpenDisplay(selectedPeer, nextSession) : data.stream;
      setData((current) => ({ ...current, session: nextSession, stream: nextStream }));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleCheckForUpdate() {
    setIsCheckingUpdate(true);
    try {
      await checkAndInstallUpdate(setUpdateStatus);
    } finally {
      setIsCheckingUpdate(false);
    }
  }

  async function handleOpenDisplayWindow() {
    if (!selectedPeer || !isConnected) return;

    setIsBusy(true);
    try {
      if (!displayPipelineReady) {
        await closeDisplayWindow();
        const nextSetupStatus = await runNativeSetup();
        setSetupStatus(nextSetupStatus);
        setDisplayWindow({ attached: false, message: nextSetupStatus.message });
        return;
      }

      const nextDisplayWindow = await openDisplayWindow({
        peerId: selectedPeer.id,
        peerAddress: frameUrlForPeer(selectedPeer),
        screenCount: Math.max(screens.length, 1),
        quality,
      });
      setDisplayWindow(nextDisplayWindow);
    } finally {
      setIsBusy(false);
    }
  }

  async function handlePairAndConnect() {
    if (!selectedPeer) return;

    const expectedCode = pairingCodeForPeer(selectedPeer.id);
    const enteredCode = pairingCode.replace(/\D/g, '');

    if (enteredCode !== expectedCode) {
      setPairingError('Code klopt niet. Gebruik de grote "Jouw pairing code" van het andere apparaat.');
      return;
    }

    const nextTrustedPeerIds = Array.from(new Set([...trustedPeerIds, selectedPeer.id]));
    saveTrustedPeerIds(nextTrustedPeerIds);
    setTrustedPeerIds(nextTrustedPeerIds);
    setPairingPeerId('');
    setPairingCode('');
    setPairingError('');

    setIsBusy(true);
    try {
      const nextSession = await connectPeer(selectedPeer.id);
      const nextStream = await startStreamAndOpenDisplay(selectedPeer, nextSession);
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
        <div className="hero-actions">
          <div className="pairing-code-card" aria-live="polite">
            <span>Jouw pairing code</span>
            <strong>{localPairingCode}</strong>
            <small>Typ deze code op het andere apparaat.</small>
          </div>
          <button className="scan-button" disabled={isScanning} onClick={() => loadEverything(true)}>
            {isScanning ? <Loader2 className="spin" size={17} /> : <RefreshCw size={17} />}
            Scan opnieuw
          </button>
        </div>
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
                  <em>{trustedPeerIds.includes(peer.id) || peer.trusted ? 'trusted' : 'pairing'}</em>
                </button>
              ))}
            </div>
          )}

          {selectedPeer && pairingPeerId === selectedPeer.id && (
            <div className="pairing-box">
              <strong>Voer de code van {selectedPeer.name} in</strong>
              <span>Open PaneLink op dat apparaat. De code staat bovenaan als "Jouw pairing code".</span>
              <input
                autoFocus
                inputMode="numeric"
                maxLength={6}
                onChange={(event) => {
                  setPairingCode(event.target.value.replace(/\D/g, '').slice(0, 6));
                  setPairingError('');
                }}
                placeholder="6 cijfers"
                value={pairingCode}
              />
              {pairingError && <small>{pairingError}</small>}
              <div>
                <button className="secondary-action" onClick={() => setPairingPeerId('')}>Annuleer</button>
                <button className="primary-action compact" disabled={pairingCode.length !== 6 || isBusy} onClick={handlePairAndConnect}>
                  {isBusy ? <Loader2 className="spin" size={16} /> : <CheckCircle2 size={16} />}
                  Pair en verbind
                </button>
              </div>
            </div>
          )}

          <button className="primary-action" disabled={isBusy || !selectedPeer} onClick={handleConnect}>
            {isBusy ? <Loader2 className="spin" size={18} /> : <Monitor size={18} />}
            {isConnected
              ? 'Verbinding stoppen'
              : selectedPeer && !selectedPeerTrusted
                ? `Pair met ${selectedPeer.name}`
                : selectedPeer
                  ? `Verbind met ${selectedPeer.name}`
                  : 'Geen apparaat gevonden'}
          </button>
        </div>

        <div className="session-panel">
          <div className="session-top">
            <div>
              <span className="section-label">Session</span>
              <h2>{isConnected ? selectedPeer?.name ?? 'Verbonden apparaat' : 'Nog niet verbonden'}</h2>
            </div>
            <strong>{receiverReady ? `${stream?.fps ?? 0} FPS` : isConnected ? 'Receiver nodig' : 'Standby'}</strong>
          </div>

          <div className={receiverReady ? 'preview-frame streaming' : isStreaming ? 'preview-frame receiver-needed' : 'preview-frame'}>
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
              <strong>
                {receiverReady ? 'Receiver window open' : isStreaming ? 'Setup nodig' : isConnected ? 'Stream starten...' : 'Wacht op verbinding'}
              </strong>
              <span>
                {receiverReady && displayPipelineReady
                  ? `${stream?.codec ?? 'H.264'} - frame ${stream?.frameId ?? 0} - ${stream?.latencyMs ?? 0} ms`
                  : receiverReady
                    ? 'Receiver staat open. Echte schermpixels wachten nog op native capture en virtual display driver.'
                    : isStreaming
                      ? setupStatus?.message ?? 'PaneLink start automatisch de native setup voordat er een display window wordt geopend.'
                      : isConnected
                        ? 'De stream-engine wordt gestart.'
                        : 'Klik links op verbinden.'}
              </span>
              <span hidden>
                {isStreaming
                  ? `${stream?.codec ?? 'H.264'} · frame ${stream?.frameId ?? 0} · ${stream?.latencyMs ?? 0} ms`
                  : isConnected
                    ? 'De stream-engine wordt gestart.'
                    : 'Klik links op verbinden.'}
              </span>
              {isConnected && (
                <button className="inline-action" disabled={isBusy} onClick={handleOpenDisplayWindow}>
                  {isBusy ? <Loader2 className="spin" size={15} /> : <Monitor size={15} />}
                  Open display
                </button>
              )}
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
          <button className="secondary-action update-action" disabled={isCheckingUpdate} onClick={handleCheckForUpdate}>
            {isCheckingUpdate ? <Loader2 className="spin" size={16} /> : <RefreshCw size={16} />}
            Check for update
          </button>
          <small className="muted">Pairing code: {localPairingCode}</small>
          <small className="muted">Update: {updateStatus.label}</small>
          <small className="muted">Peer ID: {data.capabilities?.peerId ?? 'laden...'}</small>
          <small className="muted">Display window: {displayWindow.attached ? 'open' : 'closed'}</small>
          <small className="muted">Setup: {setupStatus?.message ?? 'niet gestart'}</small>
          <small className="muted">Capture: {data.capabilities?.display.capture ?? 'laden...'}</small>
          <small className="muted">Virtual display: {data.capabilities?.display.virtualDisplay ?? 'laden...'}</small>
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

function DisplayWindow() {
  const [config, setConfig] = useState(readDisplayWindowConfig);
  const screenCount = Math.max(1, Math.min(Number(config.screenCount || 1), 3));
  const screens = Array.from({ length: screenCount }, (_, index) => index + 1);

  useEffect(() => {
    const refreshConfig = () => setConfig(readDisplayWindowConfig());
    const timer = window.setInterval(refreshConfig, 700);
    window.addEventListener('storage', refreshConfig);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener('storage', refreshConfig);
    };
  }, []);

  return (
    <main className="display-window-shell">
      <section className={`display-window-grid screen-count-${screenCount}`}>
        {screens.map((screen) => (
          <div className="display-window-screen" key={screen}>
            <span>Screen {screen}</span>
            <small>Geen desktopframes</small>
          </div>
        ))}
      </section>
      <div className="display-window-status">
        <Monitor size={34} />
        <strong>Nog geen beeld ontvangen</strong>
      <span>{config.peerId} - {config.quality}</span>
        <small>Frames: {config.peerAddress}</small>
      </div>
    </main>
  );
}

function DisplayBoot() {
  return (
    <main className="display-window-shell boot">
      <div className="display-window-status">
        <Loader2 className="spin" size={30} />
        <strong>PaneLink openen</strong>
        <small>Window wordt klaargezet.</small>
      </div>
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

function pairingCodeForPeer(peerId: string) {
  let hash = 0;
  for (const char of peerId) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }

  return String(hash % 1_000_000).padStart(6, '0');
}

function loadTrustedPeerIds() {
  try {
    return JSON.parse(window.localStorage.getItem('panelink.trustedPeerIds') ?? '[]') as string[];
  } catch {
    return [];
  }
}

function saveTrustedPeerIds(peerIds: string[]) {
  window.localStorage.setItem('panelink.trustedPeerIds', JSON.stringify(peerIds));
}

function readDisplayWindowConfig(): DisplayWindowRequest {
  const params = new URLSearchParams(window.location.search);
  const fromUrl = params.get('window') === 'display'
    ? {
        peerId: params.get('peerId') ?? 'unknown',
        peerAddress: params.get('peerAddress') ?? '',
        screenCount: Number(params.get('screens') ?? 1),
        quality: (params.get('quality') ?? 'Low latency') as StreamState['quality'],
      }
    : null;

  if (fromUrl) {
    return fromUrl;
  }

  try {
    const saved = window.localStorage.getItem('panelink.displayWindow');
    if (saved) {
      return JSON.parse(saved) as DisplayWindowRequest;
    }
  } catch {
    // Fall through to the default below.
  }

  return { peerId: 'unknown', peerAddress: '', screenCount: 1, quality: 'Low latency' };
}

function frameUrlForPeer(peer: Peer) {
  const host = peer.address.split(':')[0] || peer.address;

  return `http://${host}:48171/frame`;
}

export default App;
