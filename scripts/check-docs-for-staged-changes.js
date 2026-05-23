#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const DOC_FILES = new Set(['AGENTS.md', 'README.md', 'CONTRIBUTING.md']);
const DOC_PREFIXES = ['docs/'];

const DOC_SENSITIVE_PREFIXES = [
  '.githooks/',
  'apps/',
  'crates/',
  'packages/',
  'scripts/'
];

const DOC_SENSITIVE_FILES = [
  /^Cargo\.(toml|lock)$/,
  /^package(-lock)?\.json$/,
  /^tsconfig(\..*)?\.json$/,
  /^vite\.config\./
];

const IGNORED_PREFIXES = ['node_modules/', 'target/', 'dist/', 'build/'];

export function isDocumentationPath(filePath) {
  return DOC_FILES.has(filePath) || DOC_PREFIXES.some((prefix) => filePath.startsWith(prefix));
}

export function isDocsSensitivePath(filePath) {
  if (!filePath || IGNORED_PREFIXES.some((prefix) => filePath.startsWith(prefix))) {
    return false;
  }

  if (isDocumentationPath(filePath)) {
    return false;
  }

  return (
    DOC_SENSITIVE_PREFIXES.some((prefix) => filePath.startsWith(prefix)) ||
    DOC_SENSITIVE_FILES.some((pattern) => pattern.test(filePath))
  );
}

export function evaluateDocumentationCheck(stagedPaths, { skip = false } = {}) {
  const changedPaths = [...new Set(stagedPaths.filter(Boolean))];

  if (skip) {
    return {
      ok: true,
      reason: 'skipped',
      docsChanged: changedPaths.filter(isDocumentationPath),
      sensitiveChanged: changedPaths.filter(isDocsSensitivePath)
    };
  }

  const docsChanged = changedPaths.filter(isDocumentationPath);
  const sensitiveChanged = changedPaths.filter(isDocsSensitivePath);

  if (sensitiveChanged.length === 0 || docsChanged.length > 0) {
    return {
      ok: true,
      reason: sensitiveChanged.length === 0 ? 'no-sensitive-changes' : 'docs-updated',
      docsChanged,
      sensitiveChanged
    };
  }

  return {
    ok: false,
    reason: 'docs-missing',
    docsChanged,
    sensitiveChanged
  };
}

function stagedFiles() {
  const output = execFileSync(
    'git',
    ['diff', '--cached', '--name-only', '--diff-filter=ACMR'],
    { encoding: 'utf8' }
  );
  return output.split('\n').map((line) => line.trim()).filter(Boolean);
}

function formatList(items, limit = 12) {
  const visible = items.slice(0, limit).map((item) => `  - ${item}`);
  const remaining = items.length - visible.length;
  if (remaining > 0) {
    visible.push(`  - ...and ${remaining} more`);
  }
  return visible.join('\n');
}

export function main({ env = process.env, stdout = process.stdout, stderr = process.stderr } = {}) {
  const result = evaluateDocumentationCheck(stagedFiles(), {
    skip: env.SKILLBOX_SKIP_DOCS_CHECK === '1'
  });

  if (result.ok) {
    if (result.reason === 'skipped') {
      stdout.write('SkillBox docs check skipped by SKILLBOX_SKIP_DOCS_CHECK=1.\n');
    }
    return 0;
  }

  stderr.write(`SkillBox docs check failed.

The staged changes touch implementation or workflow files, but no project docs are staged.
Please decide whether AGENTS.md, README.md, CONTRIBUTING.md, or docs/* needs an update.

Implementation/workflow files staged:
${formatList(result.sensitiveChanged)}

Stage the matching docs update, or rerun commit with SKILLBOX_SKIP_DOCS_CHECK=1 if you checked and no docs are needed.
`);
  return 1;
}

const invokedPath = process.argv[1] ? pathToFileURL(process.argv[1]).href : '';

if (import.meta.url === invokedPath) {
  process.exitCode = main();
}
