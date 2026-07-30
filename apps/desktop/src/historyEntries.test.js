import assert from 'node:assert/strict';
import test from 'node:test';

import { groupHistoryEntriesByDay } from './historyEntries.js';

test('groups entries from the same calendar day together', () => {
  const groups = groupHistoryEntriesByDay([
    { id: 'first', timestamp: '2026-07-29T08:00:00+08:00' },
    { id: 'second', timestamp: '2026-07-29T16:30:00+08:00' },
    { id: 'third', timestamp: '2026-07-28T10:00:00+08:00' }
  ]);

  assert.equal(groups.length, 2);
  assert.deepEqual(groups.map((group) => group.entries.map((entry) => entry.id)), [
    ['first', 'second'],
    ['third']
  ]);
});

test('groups missing and invalid timestamps under Unknown date', () => {
  const groups = groupHistoryEntriesByDay([
    { id: 'missing', timestamp: '' },
    { id: 'invalid', timestamp: 'not-a-date' }
  ]);

  assert.equal(groups.length, 1);
  assert.equal(groups[0].key, 'unknown');
  assert.equal(groups[0].label, 'Unknown date');
  assert.deepEqual(groups[0].entries.map((entry) => entry.id), ['missing', 'invalid']);
});

test('accepts numeric Unix timestamps', () => {
  const timestamp = 1_725_000_000;
  const expected = new Date(timestamp * 1000);
  const expectedKey = [
    expected.getFullYear(),
    String(expected.getMonth() + 1).padStart(2, '0'),
    String(expected.getDate()).padStart(2, '0')
  ].join('-');

  const [group] = groupHistoryEntriesByDay([{ id: 'numeric', timestamp }]);

  assert.equal(group.key, expectedKey);
  assert.equal(group.entries[0].id, 'numeric');
});
