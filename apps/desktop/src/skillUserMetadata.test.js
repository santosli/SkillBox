import assert from 'node:assert/strict';
import test from 'node:test';
import {
  legacySkillUserMetadataUpdates,
  mergeSkillUserMetadataRow,
  normalizeSkillUserMetadata
} from './skillUserMetadata.js';

test('normalizes SQLite skill metadata into dashboard favorites and tags', () => {
  const metadata = normalizeSkillUserMetadata([
    { skill_name: 'beta', favorite: true, tags: [' Research Notes ', 'sync'] },
    { skillName: 'alpha', favorite: false, tags: [] }
  ]);

  assert.ok(Array.isArray(metadata.favoriteNames));
  assert.deepEqual(metadata, {
    favoriteNames: ['beta'],
    tagOverrides: { alpha: [], beta: ['research-notes', 'sync'] }
  });
});

test('builds one-time SQLite updates from legacy localStorage metadata', () => {
  assert.deepEqual(
    legacySkillUserMetadataUpdates(['beta'], {
      alpha: ['local'],
      beta: ['favorite'],
      empty: []
    }),
    [
      { skill_name: 'alpha', favorite: false, tags: ['local'] },
      { skill_name: 'beta', favorite: true, tags: ['favorite'] }
    ]
  );
});

test('merges the authoritative SQLite metadata row after a write', () => {
  assert.deepEqual(
    mergeSkillUserMetadataRow(
      ['alpha', 'demo'],
      { alpha: ['keep'], demo: ['optimistic'] },
      {
        skill_name: 'demo',
        favorite: false,
        tags: ['server-normalized']
      }
    ),
    {
      favoriteNames: ['alpha'],
      tagOverrides: {
        alpha: ['keep'],
        demo: ['server-normalized']
      }
    }
  );
});
