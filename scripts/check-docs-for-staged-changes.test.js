import assert from 'node:assert/strict';
import test from 'node:test';

import {
  evaluateDocumentationCheck,
  isDocsSensitivePath,
  isDocumentationPath
} from './check-docs-for-staged-changes.js';

test('classifies documentation paths', () => {
  assert.equal(isDocumentationPath('AGENTS.md'), true);
  assert.equal(isDocumentationPath('README.md'), true);
  assert.equal(isDocumentationPath('CONTRIBUTING.md'), true);
  assert.equal(isDocumentationPath('docs/workflows.md'), true);
  assert.equal(isDocumentationPath('apps/desktop/src/App.jsx'), false);
});

test('classifies docs-sensitive implementation paths', () => {
  assert.equal(isDocsSensitivePath('apps/desktop/src/App.jsx'), true);
  assert.equal(isDocsSensitivePath('crates/skillbox-core/src/lib.rs'), true);
  assert.equal(isDocsSensitivePath('crates/skillbox-cli/src/main.rs'), true);
  assert.equal(isDocsSensitivePath('.githooks/pre-commit'), true);
  assert.equal(isDocsSensitivePath('package.json'), true);
  assert.equal(isDocsSensitivePath('README.md'), false);
});

test('passes when staged changes are docs-only or not docs-sensitive', () => {
  assert.equal(evaluateDocumentationCheck(['README.md']).ok, true);
  assert.equal(evaluateDocumentationCheck(['notes/scratch.md']).ok, true);
});

test('requires docs when implementation changes are staged without docs', () => {
  const result = evaluateDocumentationCheck(['apps/desktop/src/App.jsx']);

  assert.equal(result.ok, false);
  assert.equal(result.reason, 'docs-missing');
  assert.deepEqual(result.sensitiveChanged, ['apps/desktop/src/App.jsx']);
});

test('passes implementation changes when docs are staged', () => {
  const result = evaluateDocumentationCheck(['apps/desktop/src/App.jsx', 'docs/workflows.md']);

  assert.equal(result.ok, true);
  assert.equal(result.reason, 'docs-updated');
});

test('allows explicit skip for commits that do not need docs', () => {
  const result = evaluateDocumentationCheck(['crates/skillbox-core/src/lib.rs'], { skip: true });

  assert.equal(result.ok, true);
  assert.equal(result.reason, 'skipped');
});
