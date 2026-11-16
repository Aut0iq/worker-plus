import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fmtMinutes, fmtClock, fmtTs } from './format.ts';

test('fmtMinutes', () => {
  assert.equal(fmtMinutes(104), '1ч 44м');
  assert.equal(fmtMinutes(0), '0м');
  assert.equal(fmtMinutes(59), '59м');
  assert.equal(fmtMinutes(120), '2ч 0м');
  assert.equal(fmtMinutes(null), '0м');
});

test('fmtClock', () => {
  assert.equal(fmtClock(3725), '01:02:05');
  assert.equal(fmtClock(59), '00:00:59');
});

test('fmtTs in Asia/Vladivostok', () => {
  // 2026-11-16T09:35:00Z -> 19:35 local (UTC+10)
  assert.equal(fmtTs('2026-11-16T09:35:00Z', 'Asia/Vladivostok'), '16.11.2026 19:35');
  assert.equal(fmtTs(null, 'Asia/Vladivostok'), '—');
});
