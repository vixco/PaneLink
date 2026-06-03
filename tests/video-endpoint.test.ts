import assert from 'node:assert/strict';
import test from 'node:test';

import { endpointForControlHost, isH264StreamEndpoint } from '../src/video-endpoint.ts';

test('detects h264 stream endpoints separately from png frames', () => {
  assert.equal(isH264StreamEndpoint('http://192.168.1.20:48170/h264?fps=60'), true);
  assert.equal(isH264StreamEndpoint('http://192.168.1.20:48171/frame'), false);
  assert.equal(isH264StreamEndpoint('webrtc+rtp://windows/panelink/session'), false);
});

test('rewrites loopback h264 endpoint to the remote control host', () => {
  assert.equal(
    endpointForControlHost('http://127.0.0.1:48170/h264?fps=60', 'http://192.168.1.20:48170'),
    'http://192.168.1.20:48170/h264?fps=60',
  );
});
