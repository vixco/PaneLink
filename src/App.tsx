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
import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import {
  addRemoteScreen,
  closeDisplayWindow,
  connectPeer,
  createVirtualDisplay,
  disconnectPeer,
  destroyVirtualDisplay,
  fetchRemoteFrame,
  getCapabilities,
  getControlServerLanUrl,
  getFrameServerLanUrl,
  getPermissions,
  getSession,
  getStreamState,
  getVideoBackend,
  getVirtualDisplayBackend,
  listAudioDevices,
  listPeers,
  openDisplayWindow,
  openRemoteDisplayWindow,
  prepareRemoteHostDisplay,
  removeRemoteScreen,
  runNativeSetup,
  scanPeers,
  startStream,
  startVideoSession,
  stopStream,
  stopVideoSession,
} from './tauri';
import { selectDisplayPipeline } from './display-routing';
import { framePollDelayMs } from './frame-timing';
import { createManualPeer } from './manual-peer';
import type {
  AudioDevice,
  Capabilities,
  DisplayWindowRequest,
  NativeSetupState,
  Peer,
  PermissionState,
  RemoteFrameResponse,
  RemoteScreen,
  SessionSnapshot,
  StreamState,
  VideoBackendReport,
  VirtualDisplayBackendReport,
  VirtualDisplaySession,
} from './types';
import { checkAndInstallUpdate, type UpdateStatus } from './updater';

type AppData = {
  peers: Peer[];
  session: SessionSnapshot | null;
  stream: StreamState | null;
  capabilities: Capabilities | null;
  audioDevices: AudioDevice[];
  permissions: PermissionState[];
  virtualDisplayBackend: VirtualDisplayBackendReport | null;
  videoBackend: VideoBackendReport | null;
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
    virtualDisplayBackend: null,
    videoBackend: null,
  });
  const [selectedPeerId, setSelectedPeerId] = useState('');
  const [quality, setQuality] = useState<StreamState['quality']>('Sharp');
  const [isBusy, setIsBusy] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [trustedPeerIds, setTrustedPeerIds] = useState<string[]>(() => loadTrustedPeerIds());
  const [pairingPeerId, setPairingPeerId] = useState('');
  const [pairingCode, setPairingCode] = useState('');
  const [pairingError, setPairingError] = useState('');
  const [manualHost, setManualHost] = useState('');
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ state: 'idle', label: 'Updates ready' });
  const [displayWindow, setDisplayWindow] = useState({ attached: false, message: 'Display window closed' });
  const [setupStatus, setSetupStatus] = useState<NativeSetupState | null>(null);
  const [virtualDisplay, setVirtualDisplay] = useState<VirtualDisplaySession | null>(null);

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
  const displayPipeline = useMemo(
    () => selectDisplayPipeline(data.videoBackend, data.capabilities),
    [data.videoBackend, data.capabilities],
  );
  const displayPipelineReady = displayPipeline.kind !== 'unavailable';
  const videoEngineMissing = isConnected && displayPipeline.kind === 'unavailable';
  const usingFrameFallback = displayPipeline.kind === 'frame-fallback';
  const receiverReady = isStreaming && displayWindow.attached;
  const selectedPeerTrusted = selectedPeer ? trustedPeerIds.includes(selectedPeer.id) || selectedPeer.trusted : false;
  const localPairingCode = data.capabilities?.peerId ? pairingCodeForPeer(data.capabilities.peerId) : 'laden...';
  const needsMacVirtualDisplay = needsVirtualDisplayForPeer(data.capabilities, selectedPeer);

  async function startStreamAndOpenDisplay(peer: Peer, nextSession: SessionSnapshot) {
    const virtualDisplayReady = await ensureVirtualDisplayForSession(peer, nextSession);
    if (!virtualDisplayReady) {
      return null;
    }

    if (displayPipeline.kind === 'unavailable') {
      await closeDisplayWindow();
      const backendMessage = data.videoBackend?.message
        || 'PaneLink kan geen display stream starten op dit apparaat.';
      const nextSetupStatus = {
        started: false,
        platform: data.capabilities?.platform ?? 'unknown',
        message: backendMessage,
        actions: data.videoBackend?.actions.length ? data.videoBackend.actions : ['Installeer de PaneLink native video engine'],
        requiresRestart: false,
      };
      setSetupStatus(nextSetupStatus);
      setDisplayWindow({
        attached: false,
        message: nextSetupStatus.message,
      });

      return null;
    }

    const nextStream = await startStream({
      peerId: peer.id,
      screenIds: nextSession.screens.map((screen) => screen.id),
      quality,
    });

    const nextDisplayWindow = await openDisplayForPeer(peer, nextSession.screens.length);
    setDisplayWindow(nextDisplayWindow);

    return nextStream;
  }

  async function ensureVirtualDisplayForSession(peer: Peer, nextSession: SessionSnapshot) {
    if (!needsVirtualDisplayForPeer(data.capabilities, peer)) {
      return true;
    }

    const backend = data.virtualDisplayBackend ?? await getVirtualDisplayBackend();
    setData((current) => ({ ...current, virtualDisplayBackend: backend }));

    if (!backend.available) {
      const message = backend.message || 'Mac virtual display backend ontbreekt.';
      setVirtualDisplay(null);
      setSetupStatus({
        started: false,
        platform: data.capabilities?.platform ?? 'macos',
        message,
        actions: backend.actions.length ? backend.actions : ['Start dit vanaf de Mac die de extra monitor nodig heeft'],
        requiresRestart: false,
      });
      setDisplayWindow({ attached: false, message });
      return false;
    }

    const targetMode = modeFromScreen(nextSession.screens[nextSession.screens.length - 1]);
    const session = await createVirtualDisplay({
      name: `PaneLink ${peer.name}`,
      width: targetMode.width,
      height: targetMode.height,
      refreshHz: targetMode.refreshHz,
    });
    setVirtualDisplay(session);

    if (!session.active) {
      const message = session.message || 'PaneLink kon geen echte virtuele Mac-monitor starten.';
      setSetupStatus({
        started: false,
        platform: data.capabilities?.platform ?? 'macos',
        message,
        actions: backend.actions,
        requiresRestart: false,
      });
      setDisplayWindow({ attached: false, message });
      return false;
    }

    return true;
  }

  async function openDisplayForPeer(peer: Peer, screenCount: number) {
    const localPlatform = data.capabilities?.platform.toLowerCase() ?? '';
    const localPeerId = data.capabilities?.peerId ?? 'local-source';
    const shouldOpenOnReceiver = localPlatform === 'macos' && peer.os === 'Windows';
    const shouldUseRemoteMacSource = localPlatform === 'windows' && peer.os === 'macOS';
    const controlAddress = shouldOpenOnReceiver ? await getControlServerLanUrl(peer.address) : controlUrlForPeer(peer);
    const targetMode = modeFromScreen(screens[Math.max(0, Math.min(screenCount, screens.length) - 1)]);

    if (displayPipeline.kind === 'frame-fallback') {
      if (shouldUseRemoteMacSource) {
        await prepareRemoteHostDisplay(controlAddress, {
          width: targetMode.width,
          height: targetMode.height,
          refreshHz: 60,
          quality,
        });
      }

      const frameUrl = shouldUseRemoteMacSource ? frameUrlForPeer(peer) : await getFrameServerLanUrl(peer.address);
      const request: DisplayWindowRequest = {
        peerId: shouldUseRemoteMacSource ? peer.id : localPeerId,
        peerAddress: frameUrl,
        controlAddress,
        videoSessionId: `frame-${Date.now()}`,
        videoTransport: 'Native frame fallback',
        videoCodec: 'PNG frame stream',
        screenCount: Math.max(screenCount, 1),
        quality,
      };

      if (shouldOpenOnReceiver) {
        return openRemoteDisplayWindow(peer.address, request, peer.id);
      }

      return openDisplayWindow(request);
    }

    const videoSession = await startVideoSession({
      sourcePeerId: shouldOpenOnReceiver ? localPeerId : peer.id,
      receiverPeerId: peer.id,
      screenCount: Math.max(screenCount, 1),
      quality,
      width: targetMode.width,
      height: targetMode.height,
      refreshHz: targetMode.refreshHz,
      controlAddress,
    });
    const request: DisplayWindowRequest = {
      peerId: shouldOpenOnReceiver ? localPeerId : peer.id,
      peerAddress: videoSession.endpoint,
      controlAddress: videoSession.controlAddress,
      videoSessionId: videoSession.id,
      videoTransport: videoSession.transport,
      videoCodec: videoSession.codec,
      screenCount: Math.max(screenCount, 1),
      quality,
    };

    if (shouldOpenOnReceiver) {
      return openRemoteDisplayWindow(peer.address, request, peer.id);
    }

    return openDisplayWindow(request);
  }

  async function loadEverything(scan = false) {
    setIsScanning(scan);
    try {
      const [peers, session, stream, capabilities, audioDevices, permissions, virtualDisplayBackend, videoBackend] = await Promise.all([
        scan ? scanPeers() : listPeers(),
        getSession(),
        getStreamState(),
        getCapabilities(),
        listAudioDevices(),
        getPermissions(),
        getVirtualDisplayBackend(),
        getVideoBackend(),
      ]);

      setData({ peers, session, stream, capabilities, audioDevices, permissions, virtualDisplayBackend, videoBackend });
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
        await stopVideoSession();
        const nextSession = await disconnectPeer();
        const nextDisplayWindow = await closeDisplayWindow();
        if (virtualDisplay?.active) {
          const closedVirtualDisplay = await destroyVirtualDisplay(virtualDisplay.id);
          setVirtualDisplay(closedVirtualDisplay);
        }
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
      setData((current) => ({ ...current, session: nextSession, stream: nextStream ?? current.stream }));
    } finally {
      setIsBusy(false);
    }
  }

  function handleAddManualHost() {
    const peer = createManualPeer(manualHost);
    if (!peer.address || peer.address === ':48170') {
      return;
    }

    setData((current) => {
      const peers = current.peers.filter((item) => item.id !== peer.id);
      return { ...current, peers: [peer, ...peers] };
    });
    setTrustedPeerIds((current) => {
      const nextTrustedPeerIds = Array.from(new Set([...current, peer.id]));
      saveTrustedPeerIds(nextTrustedPeerIds);
      return nextTrustedPeerIds;
    });
    setSelectedPeerId(peer.id);
    setManualHost('');
  }

  async function handleAddScreen() {
    if (!selectedPeer || screens.length >= 3) return;

    setIsBusy(true);
    try {
      const nextSession = await addRemoteScreen(selectedPeer.id);
      const nextStream = await startStreamAndOpenDisplay(selectedPeer, nextSession);
      setData((current) => ({ ...current, session: nextSession, stream: nextStream ?? current.stream }));
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
      setData((current) => ({ ...current, session: nextSession, stream: nextStream ?? current.stream }));
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
      if (displayPipeline.kind === 'unavailable') {
        await closeDisplayWindow();
        const nextSetupStatus = await runNativeSetup();
        setSetupStatus(nextSetupStatus);
        setDisplayWindow({ attached: false, message: nextSetupStatus.message });
        return;
      }

      const nextDisplayWindow = await openDisplayForPeer(selectedPeer, Math.max(screens.length, 1));
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
      setData((current) => ({ ...current, session: nextSession, stream: nextStream ?? current.stream }));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleRunNativeSetup() {
    setIsBusy(true);
    try {
      const nextSetupStatus = await runNativeSetup();
      setSetupStatus(nextSetupStatus);
    } finally {
      setIsBusy(false);
    }
  }

  async function handleCreateVirtualDisplay() {
    if (!selectedPeer) return;

    setIsBusy(true);
    try {
      const backend = await getVirtualDisplayBackend();
      setData((current) => ({ ...current, virtualDisplayBackend: backend }));
      if (!backend.available) {
        setSetupStatus({
          started: false,
          platform: data.capabilities?.platform ?? 'unknown',
          message: backend.message,
          actions: backend.actions,
          requiresRestart: false,
        });
        return;
      }

      const targetMode = modeFromScreen(screens[screens.length - 1]);
      const session = await createVirtualDisplay({
        name: `PaneLink ${selectedPeer.name}`,
        width: targetMode.width,
        height: targetMode.height,
        refreshHz: targetMode.refreshHz,
      });
      setVirtualDisplay(session);
      setSetupStatus({
        started: session.active,
        platform: data.capabilities?.platform ?? 'unknown',
        message: session.message,
        actions: backend.actions,
        requiresRestart: false,
      });
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

          <div className="manual-connect-box">
            <strong>Manual Mac host</strong>
            <div>
              <input
                onChange={(event) => setManualHost(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    handleAddManualHost();
                  }
                }}
                placeholder="192.168.1.42"
                value={manualHost}
              />
              <button className="secondary-action" disabled={!manualHost.trim()} onClick={handleAddManualHost}>
                Add
              </button>
            </div>
          </div>

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
            <strong>{receiverReady ? `${stream?.fps ?? 0} FPS` : videoEngineMissing ? 'Engine ontbreekt' : isConnected ? 'Receiver nodig' : 'Standby'}</strong>
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
                {receiverReady
                  ? 'Receiver window open'
                  : isStreaming
                    ? 'Receiver openen'
                    : videoEngineMissing
                      ? 'Video engine ontbreekt'
                      : isConnected
                        ? 'Stream starten...'
                        : 'Wacht op verbinding'}
              </strong>
              <span>
                {receiverReady && displayPipelineReady
                  ? `${usingFrameFallback ? 'PNG frame stream' : stream?.codec ?? 'H.264'} - frame ${stream?.frameId ?? 0} - ${stream?.latencyMs ?? 0} ms`
                  : receiverReady
                    ? 'Receiver staat open. Echte schermpixels wachten nog op native capture en virtual display driver.'
                    : isStreaming
                      ? displayWindow.message || setupStatus?.message || 'PaneLink wacht op de native video engine.'
                      : videoEngineMissing
                        ? setupStatus?.message || data.videoBackend?.message || 'Native remote-desktop video engine ontbreekt.'
                      : isConnected
                        ? 'Stream wordt gestart.'
                        : 'Klik links op verbinden.'}
              </span>
              <span hidden>
                {isStreaming
                  ? `${stream?.codec ?? 'H.264'} · frame ${stream?.frameId ?? 0} · ${stream?.latencyMs ?? 0} ms`
                  : videoEngineMissing
                    ? setupStatus?.message || data.videoBackend?.message || 'Native remote-desktop video engine ontbreekt.'
                    : isConnected
                      ? 'Stream wordt gestart.'
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
            <Monitor size={18} />
            <h2>Virtual display</h2>
          </div>
          <Metric label="Backend" value={data.virtualDisplayBackend?.backend ?? 'laden...'} />
          <Metric label="State" value={virtualDisplay?.active ? 'active' : data.virtualDisplayBackend?.state ?? 'laden...'} />
          <button className="secondary-action" disabled={isBusy || !selectedPeer || !needsMacVirtualDisplay} onClick={handleCreateVirtualDisplay}>
            {isBusy ? <Loader2 className="spin" size={16} /> : <Plus size={16} />}
            Add Mac monitor
          </button>
          <small className="muted">{virtualDisplay?.message ?? data.virtualDisplayBackend?.message ?? 'Virtual display status wordt geladen.'}</small>
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
          <button className="secondary-action update-action" disabled={isBusy} onClick={handleRunNativeSetup}>
            {isBusy ? <Loader2 className="spin" size={16} /> : <Settings size={16} />}
            Native setup
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
  const [controlError, setControlError] = useState('');
  const [frameResponses, setFrameResponses] = useState<Record<number, RemoteFrameResponse>>({});
  const [isFullscreen, setIsFullscreen] = useState(false);
  const inputSequence = useRef(0);
  const screenCount = Math.max(1, Math.min(Number(config.screenCount || 1), 3));
  const screens = Array.from({ length: screenCount }, (_, index) => index + 1);
  const isFrameSession = Boolean(config.peerAddress && config.peerAddress.includes('/frame'));
  const hasVideoSession = Boolean(config.peerAddress && !isFrameSession);
  const hasLiveFrame = Object.values(frameResponses).some((response) => response.ok && response.dataUrl);

  useEffect(() => {
    const refreshConfig = () => setConfig(readDisplayWindowConfig());
    const timer = window.setInterval(refreshConfig, 700);
    window.addEventListener('storage', refreshConfig);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener('storage', refreshConfig);
    };
  }, []);

  useEffect(() => {
    const syncFullscreenState = () => setIsFullscreen(Boolean(document.fullscreenElement));

    if (isTauriRuntime) {
      import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => getCurrentWindow().isFullscreen())
        .then(setIsFullscreen)
        .catch(() => setIsFullscreen(false));
    }

    document.addEventListener('fullscreenchange', syncFullscreenState);

    return () => {
      document.removeEventListener('fullscreenchange', syncFullscreenState);
    };
  }, []);

  useEffect(() => {
    if (!isFrameSession) {
      setFrameResponses({});
      return;
    }

    let cancelled = false;
    let timer: number | undefined;

    async function loadFrames() {
      const nextFrames: Record<number, RemoteFrameResponse> = {};

      for (let screen = 1; screen <= screenCount; screen += 1) {
        nextFrames[screen] = await fetchRemoteFrame(
          frameUrlForScreen(config.peerAddress, screen),
          config.quality,
        );
      }

      if (cancelled) return;

      setFrameResponses(nextFrames);
      const responses = Object.values(nextFrames);
      const anyFrameLoaded = responses.some((response) => response.ok && response.dataUrl);
      const firstError = responses.find((response) => !response.ok)?.message ?? '';
      setControlError(anyFrameLoaded ? '' : firstError);
      timer = window.setTimeout(loadFrames, framePollDelayMs(config.quality, anyFrameLoaded));
    }

    void loadFrames();

    return () => {
      cancelled = true;
      if (timer) {
        window.clearTimeout(timer);
      }
    };
  }, [config.peerAddress, config.quality, isFrameSession, screenCount]);

  async function handleToggleFullscreen() {
    try {
      if (isTauriRuntime) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const window = getCurrentWindow();
        const nextFullscreen = !(await window.isFullscreen());
        await window.setFullscreen(nextFullscreen);
        setIsFullscreen(nextFullscreen);
        setControlError('');
        return;
      }

      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
      setControlError('');
    } catch (error) {
      setControlError(error instanceof Error ? error.message : String(error));
    }
  }

  function sendInputEvents(events: Array<Record<string, unknown>>) {
    const inputUrl = remoteControlUrl(config.controlAddress, '/input-events');
    if (!inputUrl || events.length === 0) return;

    inputSequence.current += 1;
    const batch = {
      batchId: `display-${Date.now()}-${inputSequence.current}`,
      sequence: inputSequence.current,
      sourcePeerId: config.peerId,
      events,
    };

    void fetch(inputUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(batch),
      cache: 'no-store',
    }).catch((error) => {
      setControlError(error instanceof Error ? error.message : String(error));
    });
  }

  function pointerPosition(event: ReactPointerEvent<HTMLElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)),
      y: Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)),
    };
  }

  return (
    <main
      className="display-window-shell"
      onKeyDown={(event) => {
        if (!event.repeat) {
          sendInputEvents([keyInputEvent(event, true)]);
        }
      }}
      onKeyUp={(event) => sendInputEvents([keyInputEvent(event, false)])}
      tabIndex={0}
    >
      <section className={`display-window-grid screen-count-${screenCount}`}>
        {screens.map((screen) => (
          <div
            className="display-window-screen"
            key={screen}
            onPointerDown={(event) => {
              (event.currentTarget.closest('.display-window-shell') as HTMLElement | null)?.focus();
              event.currentTarget.setPointerCapture(event.pointerId);
              sendInputEvents([
                { type: 'pointerMove', ...pointerPosition(event) },
                { type: 'pointerButton', button: pointerButtonName(event.button), pressed: true },
              ]);
            }}
            onPointerMove={(event) => sendInputEvents([{ type: 'pointerMove', ...pointerPosition(event) }])}
            onPointerUp={(event) => {
              sendInputEvents([
                { type: 'pointerMove', ...pointerPosition(event) },
                { type: 'pointerButton', button: pointerButtonName(event.button), pressed: false },
              ]);
            }}
            onWheel={(event) => sendInputEvents([{ type: 'pointerWheel', deltaX: event.deltaX, deltaY: event.deltaY }])}
          >
            {hasVideoSession && (
              <video
                aria-label={`PaneLink remote display ${screen}`}
                autoPlay
                className="display-window-frame"
                muted
                playsInline
              />
            )}
            {isFrameSession && frameResponses[screen]?.dataUrl && (
              <img
                alt={`PaneLink remote display ${screen}`}
                className="display-window-frame"
                src={frameResponses[screen].dataUrl ?? ''}
              />
            )}
            {isFrameSession && !frameResponses[screen]?.dataUrl && (
              <div className="display-window-placeholder error">
                <Loader2 className="spin" size={24} />
                <strong>Frame wordt geladen</strong>
                <small>{frameResponses[screen]?.message || config.peerAddress}</small>
              </div>
            )}
            {!hasVideoSession && !isFrameSession && (
              <div className="display-window-placeholder error">
                <Loader2 className="spin" size={24} />
                <strong>Geen video sessie</strong>
                <small>{config.peerAddress || 'Geen video endpoint ontvangen'}</small>
              </div>
            )}
            {(hasVideoSession || isFrameSession) && (
              <div className="display-window-label">
                <span>Screen {screen}</span>
                <small>{config.videoCodec ?? 'H.264 VideoToolbox'} via {config.videoTransport ?? 'WebRTC/RTP'}</small>
              </div>
            )}
          </div>
        ))}
      </section>
      <button className={hasVideoSession || hasLiveFrame ? 'display-fullscreen-action live' : 'display-fullscreen-action'} onClick={handleToggleFullscreen}>
        {isFullscreen ? 'Venster' : 'Fullscreen'}
      </button>
      {!hasVideoSession && !isFrameSession && (
        <div className="display-window-status">
          <Monitor size={34} />
          <strong>Wachten op video sessie</strong>
          <span>{config.peerId} - {config.quality}</span>
          <small>{controlError || config.peerAddress || 'Geen video endpoint ontvangen'}</small>
        </div>
      )}
    </main>
  );
}

function frameUrlForScreen(frameUrl: string, screen: number) {
  const separator = frameUrl.includes('?') ? '&' : '?';
  return `${frameUrl}${separator}screen=${screen}`;
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
        controlAddress: params.get('controlAddress') ?? '',
        videoSessionId: params.get('videoSessionId') ?? '',
        videoTransport: params.get('videoTransport') ?? 'WebRTC/RTP',
        videoCodec: params.get('videoCodec') ?? 'H.264 VideoToolbox',
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
      return normalizeDisplayWindowRequest(JSON.parse(saved) as Partial<DisplayWindowRequest>);
    }
  } catch {
    // Fall through to the default below.
  }

  return normalizeDisplayWindowRequest({});
}

function normalizeDisplayWindowRequest(request: Partial<DisplayWindowRequest>): DisplayWindowRequest {
  return {
    peerId: request.peerId ?? 'unknown',
    peerAddress: request.peerAddress ?? '',
    controlAddress: request.controlAddress ?? '',
    videoSessionId: request.videoSessionId ?? '',
    videoTransport: request.videoTransport ?? 'WebRTC/RTP',
    videoCodec: request.videoCodec ?? 'H.264 VideoToolbox',
    screenCount: Number(request.screenCount ?? 1),
    quality: request.quality ?? 'Low latency',
  };
}

function controlUrlForPeer(peer: Peer) {
  const address = peer.address.trim();
  const host = address.startsWith('[')
    ? address.slice(1, address.indexOf(']'))
    : address.split(':')[0] || address;

  return `http://${host}:48170`;
}

function frameUrlForPeer(peer: Peer) {
  const address = peer.address.trim();
  const host = address.startsWith('[')
    ? address.slice(1, address.indexOf(']'))
    : address.split(':')[0] || address;

  return `http://${host}:48171/frame`;
}

function needsVirtualDisplayForPeer(capabilities: Capabilities | null, peer: Peer | undefined) {
  return capabilities?.platform.toLowerCase() === 'macos' && peer?.os === 'Windows';
}

function modeFromScreen(screen: RemoteScreen | undefined) {
  const resolution = screen?.fittedResolution || screen?.nativeResolution || '';
  const size = resolution.match(/(\d+)\s*x\s*(\d+)/i);
  const refresh = resolution.match(/@\s*(\d+)/);

  return {
    width: Number(size?.[1] ?? 1920),
    height: Number(size?.[2] ?? 1080),
    refreshHz: Number(refresh?.[1] ?? 60),
  };
}

function pointerButtonName(button: number) {
  switch (button) {
    case 0:
      return 'primary';
    case 1:
      return 'auxiliary';
    case 2:
      return 'secondary';
    case 3:
      return 'back';
    case 4:
      return 'forward';
    default:
      return { other: String(button) };
  }
}

function keyInputEvent(event: ReactKeyboardEvent<HTMLElement>, pressed: boolean) {
  return {
    type: 'key',
    code: {
      physical: event.code,
      logical: event.key.length === 1 ? event.key : null,
    },
    pressed,
    modifiers: {
      shift: event.shiftKey,
      control: event.ctrlKey,
      alt: event.altKey,
      meta: event.metaKey,
    },
  };
}

function remoteControlUrl(controlAddress: string, path: string) {
  try {
    const url = new URL(controlAddress);
    url.pathname = path;
    url.search = '';
    url.hash = '';
    return url.toString();
  } catch {
    return '';
  }
}

export default App;
