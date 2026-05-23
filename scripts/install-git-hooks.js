#!/usr/bin/env node
import { execFileSync } from 'node:child_process';

const strictInstall = process.env.npm_lifecycle_event === 'hooks:install' || process.argv.includes('--strict');

function runGit(args) {
  return execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}

let insideWorkTree = 'false';

try {
  insideWorkTree = runGit(['rev-parse', '--is-inside-work-tree']).trim();
} catch {
  process.stdout.write('Skipping Git hooks setup outside a Git worktree.\n');
  process.exit(0);
}

if (insideWorkTree !== 'true') {
  process.stdout.write('Skipping Git hooks setup outside a Git worktree.\n');
  process.exit(0);
}

try {
  const currentHooksPath = runGit(['config', '--get', 'core.hooksPath']).trim();
  if (currentHooksPath === '.githooks') {
    process.stdout.write('Git hooks path already set to .githooks\n');
    process.exit(0);
  }
} catch {
  // Missing config is expected before the first install.
}

try {
  runGit(['config', 'core.hooksPath', '.githooks']);
  process.stdout.write('Git hooks path set to .githooks\n');
} catch {
  process.stderr.write('Unable to set Git hooks path. Run `git config core.hooksPath .githooks` from the repository root.\n');
  process.exitCode = strictInstall ? 1 : 0;
}
