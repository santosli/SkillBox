#!/usr/bin/env node
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

export const ALLOWED_TYPES = new Set([
  'feat',
  'fix',
  'docs',
  'test',
  'refactor',
  'chore',
  'build',
  'ci',
  'perf',
  'style'
]);

export const ALLOWED_SCOPES = new Set([
  'desktop',
  'core',
  'cli',
  'scan',
  'import',
  'docs',
  'hooks',
  'github'
]);

const VAGUE_SUMMARIES = new Set([
  'update',
  'updates',
  'fix',
  'fixes',
  'fix stuff',
  'improve',
  'improvements',
  'change',
  'changes',
  'misc',
  'stuff'
]);

function firstSubjectLine(message) {
  return String(message)
    .split('\n')
    .map((line) => line.trim())
    .find((line) => line && !line.startsWith('#')) ?? '';
}

function normalizedSummary(summary) {
  return summary.trim().toLowerCase().replace(/[.!?]+$/, '');
}

export function evaluateCommitMessage(message) {
  const subject = firstSubjectLine(message);

  if (!subject) {
    return { ok: false, reason: 'empty' };
  }

  const autosquash = /^(fixup|squash)! (.+)$/.exec(subject);
  if (autosquash) {
    const nested = evaluateCommitMessage(autosquash[2]);
    return nested.ok
      ? { ok: true, reason: 'autosquash' }
      : { ...nested, reason: `autosquash-${nested.reason}` };
  }

  const match = /^(?<type>[a-z]+)\((?<scope>[a-z][a-z0-9-]*)\): (?<summary>.+)$/.exec(subject);
  if (!match) {
    return { ok: false, reason: 'invalid-format', subject };
  }

  const { type, scope, summary } = match.groups;

  if (!ALLOWED_TYPES.has(type)) {
    return { ok: false, reason: 'invalid-type', type, scope, summary };
  }

  if (!ALLOWED_SCOPES.has(scope)) {
    return { ok: false, reason: 'invalid-scope', type, scope, summary };
  }

  if (!summary.trim()) {
    return { ok: false, reason: 'empty-summary', type, scope };
  }

  if (VAGUE_SUMMARIES.has(normalizedSummary(summary))) {
    return { ok: false, reason: 'vague-summary', type, scope, summary };
  }

  return { ok: true, type, scope, summary: summary.trim() };
}

function readCommitMessageFile(filePath) {
  if (!filePath) {
    return '';
  }
  return readFileSync(filePath, 'utf8');
}

export function main({ argv = process.argv, stderr = process.stderr } = {}) {
  const message = readCommitMessageFile(argv[2]);
  const result = evaluateCommitMessage(message);

  if (result.ok) {
    return 0;
  }

  stderr.write(`SkillBox commit message check failed (${result.reason}).

Use Conventional Commits for every commit message.

Format:
<type>(<scope>): <summary>

Allowed types:
feat, fix, docs, test, refactor, chore, build, ci, perf, style

Allowed scopes:
desktop, core, cli, scan, import, docs, hooks, github

Examples:
feat(scan): group imported and system candidates
chore(hooks): add staged docs update check
fix(import): skip system skills during import review
`);
  return 1;
}

const invokedPath = process.argv[1] ? pathToFileURL(process.argv[1]).href : '';

if (import.meta.url === invokedPath) {
  process.exitCode = main();
}
