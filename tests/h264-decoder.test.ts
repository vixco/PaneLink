import assert from 'node:assert/strict';
import test from 'node:test';

import { h264DecoderConfig, isH264KeyPacket, readH264Packets } from '../src/h264-decoder.ts';

test('configures WebCodecs for OpenH264 Annex B packets', () => {
  assert.deepEqual(h264DecoderConfig(), {
    codec: 'avc1.42E01F',
    optimizeForLatency: true,
    avc: { format: 'annexb' },
  });
});

test('reads length-prefixed h264 packets and keeps incomplete tail', () => {
  const packet = new Uint8Array([0, 0, 0, 1, 0x65, 1, 2, 3]);
  const buffer = new Uint8Array([0, 0, 0, packet.length, ...packet, 0, 0]);

  const result = readH264Packets(buffer);

  assert.equal(result.packets.length, 1);
  assert.deepEqual([...result.packets[0]], [...packet]);
  assert.deepEqual([...result.remaining], [0, 0]);
});

test('detects key packets from sps or idr nal units', () => {
  assert.equal(isH264KeyPacket(new Uint8Array([0, 0, 0, 1, 0x67])), true);
  assert.equal(isH264KeyPacket(new Uint8Array([0, 0, 1, 0x65])), true);
  assert.equal(isH264KeyPacket(new Uint8Array([0, 0, 1, 0x41])), false);
});
