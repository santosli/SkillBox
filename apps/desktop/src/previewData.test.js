import assert from 'node:assert/strict';
import test from 'node:test';
import {
  previewHistory,
  previewImportCandidates,
  previewImportCandidateGroups,
  previewImportCollections,
  previewSkills,
  previewUsageRankings,
  publicPreviewRequested
} from './previewData.js';

test('public preview is opt-in and uses privacy-safe deterministic fixtures', () => {
  assert.equal(publicPreviewRequested('?public-preview=1'), true);
  assert.equal(publicPreviewRequested('?public-preview=0'), false);
  assert.equal(publicPreviewRequested(''), false);

  const fixtureText = JSON.stringify({
    history: previewHistory(),
    imports: previewImportCandidates,
    rankings: previewUsageRankings(),
    skills: previewSkills
  });

  assert.doesNotMatch(fixtureText, /\/Users\/santos|prompt_excerpt|personal-wiki|black-cat/i);
  assert.match(fixtureText, /confirmed_count/);
  assert.match(fixtureText, /inferred_count/);
  assert.match(fixtureText, /reference_count/);
});

test('public preview shows a multi-child GitHub collection with explicit review states', () => {
  const collection = previewImportCollections.find(({ sourceKind }) => sourceKind === 'github_remote');
  assert.ok(collection);
  assert.equal(collection.requestedReference, 'main');
  assert.match(collection.reviewedHeadSha, /^[0-9a-f]{40}$/);
  assert.equal(collection.children.length, 5);
  assert.deepEqual(
    collection.children.map(({ importStatus, conflict }) => ({ importStatus, conflict: Boolean(conflict) })),
    [
      { importStatus: 'importable', conflict: false },
      { importStatus: 'importable', conflict: false },
      { importStatus: 'importable', conflict: false },
      { importStatus: 'importable', conflict: true },
      { importStatus: 'invalid', conflict: true }
    ]
  );
  assert.equal(
    collection.children.filter(({ isSelected }) => isSelected).length,
    1
  );
  assert.ok(previewImportCandidateGroups.some(({ id }) => id === 'skill-prompt-linter'));
  assert.match(JSON.stringify(collection), /github\.com\/skillbox-labs\/skillbox-workflows/);
  assert.doesNotMatch(JSON.stringify(collection), /\/Users\/santos|\/private\/|prompt_excerpt/i);
});
