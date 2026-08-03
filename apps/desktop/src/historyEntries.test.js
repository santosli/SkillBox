import assert from 'node:assert/strict';
import test from 'node:test';

import {
  filterHistoryEntries,
  groupHistoryEntriesByDay,
  isHistoryRequestCurrent,
  historyRequestForFilter
} from './historyEntries.js';

test('maps History filters to bounded server-side queries', () => {
  assert.deepEqual(historyRequestForFilter('all'), { limit: 200 });
  assert.deepEqual(historyRequestForFilter('skill_usage'), {
    limit: 200,
    kind: 'skill_usage'
  });
  assert.deepEqual(historyRequestForFilter('usage_reference'), {
    limit: 200,
    kind: 'usage_reference'
  });
  assert.deepEqual(historyRequestForFilter('operation'), {
    limit: 200,
    kind: 'operation'
  });
});

test('preview history filtering uses the same selected-kind contract', () => {
  const entries = [
    { id: 'call', kind: 'skill_usage' },
    { id: 'reference', kind: 'usage_reference' },
    { id: 'operation', kind: 'operation' }
  ];

  assert.deepEqual(
    filterHistoryEntries(entries, 'usage_reference').map((entry) => entry.id),
    ['reference']
  );
  assert.deepEqual(filterHistoryEntries(entries, 'all'), entries);
});

test('stale History responses and errors cannot pass the request generation gate', () => {
  assert.equal(isHistoryRequestCurrent(2, 1), false);
  assert.equal(isHistoryRequestCurrent(2, 2), true);
});

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
