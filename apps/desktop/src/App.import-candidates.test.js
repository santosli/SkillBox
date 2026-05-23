import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeImportCandidate } from './importCandidates.js';

test('normalizes backend is_selected false without selecting importable candidate', () => {
  const candidate = normalizeImportCandidate({
    name: 'system',
    description: 'System skill',
    source_path: '/Users/example/.codex/skills/.system/system',
    source_root: '/Users/example/.codex/skills/.system',
    suggested_type: 'remote',
    import_status: 'importable',
    is_selected: false,
    conflict: null
  });

  assert.equal(candidate.isSelected, false);
});
