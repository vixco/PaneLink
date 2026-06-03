import { invoke } from '@tauri-apps/api/core';
import {
  fallbackCapabilities,
  fallbackDevices,
  fallbackPeers,
  fallbackPermissions,
  fallbackSession,
  fallbackStreamState,
  fallbackVideoBackend,
  fallbackVirtualDisplayBackend,
} from './fixtures';
import type {
  AudioDevice,
  Capabilities,
  DisplayWindowRequest,
  DisplayWindowState,
  NativeSetupState,
  Peer,
  PermissionState,
  RemoteDisplayResponse,
  RemoteFrameResponse,
  RemoteScreen,
  SessionSnapshot,
  StartStreamRequest,
  StreamState,
  VideoBackendReport,
  VideoSession,
  VideoSessionRequest,
  VirtualDisplayBackendReport,
  VirtualDisplayRequest,
  VirtualDisplaySession,
} from './types';

const isTauri = '__TAURI_INTERNALS__' in window;
let browserSession: SessionSnapshot = fallbackSession;
let browserStream: StreamState = fallbackStreamState;
let browserDisplayWindow: Window | null = null;
let browserVideoSession: VideoSession | null = null;

function withNow<T extends { updatedAt?: string }>(value: T): T {
  return { ...value, updatedAt: new Date().toISOString() };
}

function screenResolution(screen: RemoteScreen) {
  const match = screen.fittedResolution.match(/(\d+)\s*x\s*(\d+)/i);
  return {
    width: Number(match?.[1] ?? 2560),
    height: Number(match?.[2] ?? 1440),
  };
}

function streamForSession(request: StartStreamRequest, session: SessionSnapshot): StreamState {
  const firstScreen = session.screens.find((screen) => request.screenIds.includes(screen.id)) ?? session.screens[0];
  const resolution = firstScreen ? screenResolution(firstScreen) : { width: 2560, height: 1440 };
  const multiplier = Math.max(request.screenIds.length, 1);
  const qualityFps = 60;

  return withNow({
    ...browserStream,
    status: 'streaming',
    activePeerId: request.peerId,
    screenIds: request.screenIds,
    codec: 'H.264 OpenH264',
    transport: 'WebRTC',
    quality: request.quality,
    width: resolution.width * multiplier,
    height: resolution.height,
    targetFps: qualityFps,
    fps: qualityFps,
    bitrateMbps: Math.round(session.bitrateMbps * multiplier * (request.quality === 'Sharp' ? 1.25 : 1)),
    latencyMs: session.latencyMs,
    jitterMs: 1.8,
    packetLoss: session.packetLoss,
    frameId: browserStream.frameId + 1,
    audioActive: true,
    microphoneActive: true,
    error: null,
  });
}

async function call<T>(command: string, fallback: T, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    await new Promise((resolve) => setTimeout(resolve, 180));
    return fallback;
  }

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    console.warn(`PaneLink command failed: ${command}`, error);
    return fallback;
  }
}

export function getCapabilities() {
  return call<Capabilities>('get_capabilities', fallbackCapabilities);
}

export function listPeers() {
  return call<Peer[]>('list_peers', isTauri ? [] : fallbackPeers);
}

export function scanPeers() {
  return call<Peer[]>('scan_peers', isTauri ? [] : fallbackPeers);
}

export function getSession() {
  return call<SessionSnapshot>('get_session_snapshot', browserSession);
}

export function listAudioDevices() {
  return call<AudioDevice[]>('list_audio_devices', fallbackDevices);
}

export function getPermissions() {
  return call<PermissionState[]>('get_permissions', fallbackPermissions);
}

export function connectPeer(peerId: string) {
  if (!isTauri) {
    return new Promise<SessionSnapshot>((resolve) =>
      setTimeout(() => {
        browserSession = {
          ...browserSession,
          status: 'connected',
          activePeerId: peerId,
          screens: browserSession.screens.map((screen) => ({ ...screen, status: 'connected' })),
        };
        resolve(browserSession);
      }, 350),
    );
  }

  return invoke<SessionSnapshot>('connect_peer', { peerId });
}

export function disconnectPeer() {
  if (!isTauri) {
    return new Promise<SessionSnapshot>((resolve) =>
      setTimeout(() => {
        browserStream = withNow({ ...browserStream, status: 'idle', activePeerId: null, screenIds: [], fps: 0, bitrateMbps: 0 });
        browserSession = {
          ...browserSession,
          status: 'ready',
          activePeerId: null,
          screens: browserSession.screens.map((screen) => ({ ...screen, status: 'ready' })),
        };
        resolve(browserSession);
      }, 180),
    );
  }

  return invoke<SessionSnapshot>('disconnect_peer');
}

export function getStreamState() {
  if (!isTauri && browserStream.status === 'streaming') {
    browserStream = withNow({
      ...browserStream,
      frameId: browserStream.frameId + Math.max(Math.round(browserStream.fps / 2), 1),
      latencyMs: Math.max(4, browserSession.latencyMs + Math.round(Math.sin(Date.now() / 900) * 2)),
      jitterMs: Number((1.4 + Math.abs(Math.sin(Date.now() / 700))).toFixed(1)),
    });
  }

  return call<StreamState>('get_stream_state', browserStream);
}

export function startStream(request: StartStreamRequest) {
  if (!isTauri) {
    return new Promise<StreamState>((resolve) =>
      setTimeout(() => {
        browserStream = streamForSession(request, browserSession);
        resolve(browserStream);
      }, 240),
    );
  }

  return call<StreamState>('start_stream', streamForSession(request, browserSession), { request });
}

export function stopStream() {
  if (!isTauri) {
    return new Promise<StreamState>((resolve) =>
      setTimeout(() => {
        browserStream = withNow({
          ...browserStream,
          status: 'idle',
          activePeerId: null,
          screenIds: [],
          fps: 0,
          bitrateMbps: 0,
          frameId: browserStream.frameId + 1,
        });
        resolve(browserStream);
      }, 140),
    );
  }

  return call<StreamState>('stop_stream', withNow({ ...browserStream, status: 'idle' }));
}

export function openDisplayWindow(request: DisplayWindowRequest) {
  saveDisplayWindowRequest(request);

  if (!isTauri) {
    const params = new URLSearchParams({
      window: 'display',
      peerId: request.peerId,
      peerAddress: request.peerAddress,
      controlAddress: request.controlAddress,
      videoSessionId: request.videoSessionId ?? '',
      videoTransport: request.videoTransport ?? 'H.264 LAN stream',
      videoCodec: request.videoCodec ?? 'H.264 OpenH264',
      screens: String(Math.max(1, Math.min(request.screenCount, 3))),
      quality: request.quality,
    });
    browserDisplayWindow = window.open(`/?${params.toString()}`, 'panelink-display', 'popup,width=1280,height=720');

    return Promise.resolve<DisplayWindowState>({
      attached: Boolean(browserDisplayWindow),
      message: browserDisplayWindow ? 'Display window opened' : 'Display window was blocked by the browser',
    });
  }

  return call<DisplayWindowState>(
    'open_display_window',
    { attached: false, message: 'Display window could not be opened' },
    { request },
  ).then(() => ({ attached: true, message: 'Display window opened' }));
}

export function openRemoteDisplayWindow(receiverAddress: string, request: DisplayWindowRequest, receiverPeerId?: string) {
  if (!isTauri) {
    return openDisplayWindow(request);
  }

  return call<RemoteDisplayResponse>(
    'open_remote_display_window',
    { ok: false, message: 'Receiver display command failed' },
    {
      receiverAddress,
      receiverPeerId,
      request,
    },
  ).then((response) => ({
    attached: response.ok,
    message: response.message,
  }));
}

export function closeDisplayWindow() {
  if (!isTauri) {
    browserDisplayWindow?.close();
    browserDisplayWindow = null;
    return Promise.resolve<DisplayWindowState>({ attached: false, message: 'Display window closed' });
  }

  return call<DisplayWindowState>('close_display_window', { attached: false, message: 'Display window closed' })
    .then(() => ({ attached: false, message: 'Display window closed' }));
}

export function runNativeSetup() {
  return call<NativeSetupState>('run_native_setup', {
    started: false,
    platform: 'browser',
    message: 'Native setup is only available in the installed PaneLink app.',
    actions: [],
    requiresRestart: false,
  });
}

export function getVirtualDisplayBackend() {
  return call<VirtualDisplayBackendReport>('get_virtual_display_backend', fallbackVirtualDisplayBackend);
}

export function getVideoBackend() {
  return call<VideoBackendReport>('get_video_backend', fallbackVideoBackend);
}

export function startVideoSession(request: VideoSessionRequest) {
  const fallback: VideoSession = {
    id: `browser-video-${Date.now()}`,
    active: true,
    endpoint: `http://127.0.0.1:48170/h264?screens=${request.screenCount}`,
    controlAddress: request.controlAddress,
    transport: 'H.264 LAN stream',
    codec: 'H.264 OpenH264',
    quality: request.quality,
    targetFps: 60,
    targetBitrateMbps: request.screenCount * (request.quality === 'Sharp' ? 52 : request.quality === 'Balanced' ? 36 : 28),
    screenCount: request.screenCount,
    width: request.width,
    height: request.height,
    message: 'Browser preview video session negotiated.',
  };

  if (!isTauri) {
    browserVideoSession = fallback;
    return Promise.resolve(fallback);
  }

  return call<VideoSession>('start_video_session', fallback, { request });
}

export function getCurrentVideoSession() {
  return call<VideoSession | null>('get_current_video_session', browserVideoSession);
}

export function stopVideoSession() {
  const fallback = browserVideoSession ? { ...browserVideoSession, active: false, message: 'Video session stopped' } : null;
  browserVideoSession = null;
  return call<VideoSession | null>('stop_video_session', fallback);
}

export function createVirtualDisplay(request: VirtualDisplayRequest) {
  return call<VirtualDisplaySession>(
    'create_virtual_display',
    {
      id: '',
      active: false,
      backend: fallbackVirtualDisplayBackend.backend,
      displayName: request.name || 'PaneLink Virtual Display',
      width: request.width || 1920,
      height: request.height || 1080,
      refreshHz: request.refreshHz || 60,
      message: fallbackVirtualDisplayBackend.message,
    },
    { request },
  );
}

export function destroyVirtualDisplay(id: string) {
  return call<VirtualDisplaySession>(
    'destroy_virtual_display',
    {
      id,
      active: false,
      backend: fallbackVirtualDisplayBackend.backend,
      displayName: 'PaneLink Virtual Display',
      width: 1920,
      height: 1080,
      refreshHz: 60,
      message: 'Virtual display closed',
    },
    { id },
  );
}

export type HostDisplayPrepareResponse = {
  ok: boolean;
  frameUrl: string;
  h264Stream: H264StreamSession | null;
  virtualDisplay: VirtualDisplaySession | null;
  message: string;
};

export type H264StreamSession = {
  active: boolean;
  endpoint: string;
  port: number;
  transport: string;
  codec: string;
  targetFps: number;
  targetBitrateMbps: number;
  message: string;
};

export async function prepareRemoteHostDisplay(
  controlAddress: string,
  request: Pick<VirtualDisplayRequest, 'width' | 'height' | 'refreshHz'> & { quality: StreamState['quality'] },
) {
  const url = new URL('/prepare-host-display', controlAddress);
  url.searchParams.set('width', String(request.width));
  url.searchParams.set('height', String(request.height));
  url.searchParams.set('refreshHz', String(request.refreshHz));
  url.searchParams.set('quality', request.quality);

  const response = await fetch(url.toString(), { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(await response.text());
  }

  return response.json() as Promise<HostDisplayPrepareResponse>;
}

export function getFrameServerUrl() {
  return call<string>('get_frame_server_url', 'http://127.0.0.1:48171/frame');
}

export function getFrameServerLanUrl(peerAddress?: string) {
  if (isTauri) {
    return peerAddress
      ? invoke<string>('get_frame_server_lan_url_for_peer', { peerAddress })
      : invoke<string>('get_frame_server_lan_url');
  }

  return Promise.resolve('http://127.0.0.1:48171/frame');
}

export function getControlServerLanUrl(peerAddress?: string) {
  if (isTauri) {
    return peerAddress
      ? invoke<string>('get_control_server_lan_url_for_peer', { peerAddress })
      : invoke<string>('get_control_server_lan_url');
  }

  return Promise.resolve('http://127.0.0.1:48170');
}

export function getDisplayFrameImageUrl(url: string, nonce: number, quality: StreamState['quality']) {
  if (!url) return '';

  const frameUrl = withFrameQuality(url, quality);

  if (isTauri) {
    return `http://127.0.0.1:48170/frame-proxy?url=${encodeURIComponent(frameUrl)}&t=${nonce}`;
  }

  const separator = frameUrl.includes('?') ? '&' : '?';
  return `${frameUrl}${separator}panelinkFrame=${nonce}`;
}

function withFrameQuality(url: string, quality: StreamState['quality']) {
  const separator = url.includes('?') ? '&' : '?';
  return `${url}${separator}quality=${encodeURIComponent(quality)}`;
}

export async function fetchRemoteFrame(url: string, quality?: StreamState['quality']) {
  const frameUrl = quality ? withFrameQuality(url, quality) : url;

  if (!isTauri) {
    try {
      const response = await fetch(frameUrl, { cache: 'no-store' });
      if (!response.ok) {
        return {
          ok: false,
          statusCode: response.status,
          contentType: response.headers.get('content-type') ?? '',
          dataUrl: null,
          message: await response.text(),
        } satisfies RemoteFrameResponse;
      }

      const blob = await response.blob();
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(blob);
      });

      return {
        ok: true,
        statusCode: response.status,
        contentType: blob.type || response.headers.get('content-type') || 'image/png',
        dataUrl,
        message: `Frame loaded from ${frameUrl}`,
      } satisfies RemoteFrameResponse;
    } catch (error) {
      return {
        ok: false,
        statusCode: 0,
        contentType: '',
        dataUrl: null,
        message: error instanceof Error ? error.message : String(error),
      } satisfies RemoteFrameResponse;
    }
  }

  return call<RemoteFrameResponse>(
    'fetch_remote_frame',
    {
      ok: false,
      statusCode: 0,
      contentType: '',
      dataUrl: null,
      message: 'Remote frame command failed',
    },
    { url: frameUrl },
  );
}

export function addRemoteScreen(peerId: string) {
  const index = browserSession.screens.length;
  const nextScreen: RemoteScreen = {
    id: `screen-${index + 1}`,
    name: `Remote screen ${index + 1}`,
    role: index === 0 ? 'primary' : 'extended',
    sourceDisplay: `Virtual Display ${index + 1}`,
    targetDisplay: `Windows Display ${index + 1}`,
    nativeResolution: index === 1 ? '1920 x 1080 @ 144 Hz' : '1440 x 900 @ 60 Hz',
    fittedResolution: index === 1 ? '1920 x 1080 @ 120 Hz' : '1440 x 900 @ 60 Hz',
    scaleMode: 'auto-fit',
    status: browserSession.status === 'connected' ? 'connected' : 'ready',
  };
  const fallback = { ...browserSession, activePeerId: peerId, screens: [...browserSession.screens, nextScreen] };

  if (!isTauri) {
    return new Promise<SessionSnapshot>((resolve) =>
      setTimeout(() => {
        browserSession = fallback;
        resolve(browserSession);
      }, 180),
    );
  }

  return call<SessionSnapshot>('add_remote_screen', fallback, { peerId });
}

export function removeRemoteScreen(screenId: string) {
  const nextScreens = browserSession.screens.length === 1
    ? browserSession.screens
    : browserSession.screens.filter((screen) => screen.id !== screenId);
  const fallback = { ...browserSession, screens: nextScreens };

  if (!isTauri) {
    return new Promise<SessionSnapshot>((resolve) =>
      setTimeout(() => {
        browserSession = fallback;
        browserStream = withNow({
          ...browserStream,
          screenIds: browserStream.screenIds.filter((id) => id !== screenId),
        });
        resolve(browserSession);
      }, 140),
    );
  }

  return call<SessionSnapshot>('remove_remote_screen', fallback, { screenId });
}

function saveDisplayWindowRequest(request: DisplayWindowRequest) {
  try {
    window.localStorage.setItem('panelink.displayWindow', JSON.stringify(request));
  } catch (error) {
    console.warn('PaneLink display config could not be saved', error);
  }
}
