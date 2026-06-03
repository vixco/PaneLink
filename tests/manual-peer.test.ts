import assert from 'node:assert/strict';
import test from 'node:test';

import { createManualPeer } from '../src/manual-peer.ts';

test('creates a macOS peer from a bare LAN IP', () => {
  const peer = createManualPeer('192.168.1.42');

  assert.equal(peer.id, 'manual:192.168.1.42');
  assert.equal(peer.os, 'macOS');
  assert.equal(peer.address, '192.168.1.42:48170');
});

test('accepts an explicit control port', () => {
  const peer = createManualPeer('192.168.1.42:48170');

  assert.equal(peer.address, '192.168.1.42:48170');
});

test('strips http URL syntax for manual hosts', () => {
  const peer = createManualPeer('http://192.168.1.42:48170/health');

  assert.equal(peer.address, '192.168.1.42:48170');
});
