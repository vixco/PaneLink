import assert from 'node:assert/strict';
import test from 'node:test';

import { selectDisplayPipeline } from '../src/display-routing.ts';

test('uses frame fallback when native video is unavailable but capture is available', () => {
  const pipeline = selectDisplayPipeline(
    { canStartSourceStream: false },
    { display: { capture: 'available' } },
  );

  assert.equal(pipeline.kind, 'frame-fallback');
});

test('uses native video when the source engine is available', () => {
  const pipeline = selectDisplayPipeline(
    { canStartSourceStream: true },
    { display: { capture: 'available' } },
  );

  assert.equal(pipeline.kind, 'native-video');
});

test('reports unavailable when neither video nor capture can run', () => {
  const pipeline = selectDisplayPipeline(
    { canStartSourceStream: false },
    { display: { capture: 'stub' } },
  );

  assert.equal(pipeline.kind, 'unavailable');
});
