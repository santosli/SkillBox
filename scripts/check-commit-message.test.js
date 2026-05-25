import assert from 'node:assert/strict';
import test from 'node:test';

import { evaluateCommitMessage } from './check-commit-message.js';

test('accepts scoped conventional commit messages', () => {
  assert.equal(
    evaluateCommitMessage('feat(desktop): refresh dashboard skill statuses').ok,
    true
  );
  assert.equal(
    evaluateCommitMessage('fix(import): skip system skills during import review').ok,
    true
  );
});

test('rejects commit messages without a scope', () => {
  const result = evaluateCommitMessage('feat: refresh dashboard skill statuses');

  assert.equal(result.ok, false);
  assert.equal(result.reason, 'invalid-format');
});

test('rejects unsupported types and vague summaries', () => {
  assert.equal(evaluateCommitMessage('wip(desktop): refresh dashboard').ok, false);
  assert.equal(evaluateCommitMessage('fix(desktop): fix stuff').ok, false);
});

test('accepts generated fixup and squash commits for interactive workflows', () => {
  assert.equal(evaluateCommitMessage('fixup! feat(desktop): refresh dashboard').ok, true);
  assert.equal(evaluateCommitMessage('squash! fix(import): skip system skills').ok, true);
});
