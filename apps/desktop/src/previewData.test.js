import assert from 'node:assert/strict';
import test from 'node:test';
import {
  previewHistory,
  previewImportCandidates,
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
