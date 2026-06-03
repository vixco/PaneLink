import assert from 'node:assert/strict';
import test from 'node:test';

import { framePollDelayMs } from '../src/frame-timing.ts';

test('polls sharp frame mode at 60 fps cadence', () => {
  assert.equal(framePollDelayMs('Sharp', true), 16);
});

test('backs off when frames are not available yet', () => {
  assert.equal(framePollDelayMs('Sharp', false), 1000);
});
