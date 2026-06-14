#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const RELEASE_WORKFLOW = 'release.yml';
const REPOSITORY = 'santosli/SkillBox';
const TAP_REPOSITORY = 'https://github.com/santosli/homebrew-tap';
const CASK_PATH = 'packaging/homebrew/Casks/skillbox.rb';

const VERSION_FILES = [
  'package.json',
  'apps/desktop/package.json',
  'package-lock.json',
  'apps/desktop/src-tauri/tauri.conf.json',
  'apps/desktop/src-tauri/Cargo.toml',
  'crates/skillbox-cli/Cargo.toml',
  'crates/skillbox-core/Cargo.toml',
  'crates/skillbox-git/Cargo.toml',
  'crates/skillbox-github/Cargo.toml',
  'Cargo.lock',
  'CHANGELOG.md',
  'README.md',
  'README.zh-CN.md',
  'SECURITY.md',
  'docs/release.md',
  'docs/roadmap.md',
  '.github/ISSUE_TEMPLATE/bug_report.yml',
  '.github/ISSUE_TEMPLATE/installation_problem.yml'
];

const CHECKS = [
  ['cargo', ['fmt', '--check']],
  ['npm', ['test']],
  ['npm', ['--workspace', 'apps/desktop', 'run', 'build']],
  ['cargo', ['clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings']],
  ['cargo', ['test', '--offline']],
  ['npm', ['audit', '--audit-level=high']],
  ['cargo', ['audit']],
  ['git', ['diff', '--check']]
];

export function normalizeVersion(input) {
  const version = String(input || '').trim().replace(/^v/, '');
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Release version must be semantic, for example 0.2.1: ${input || ''}`);
  }
  return version;
}

export function releaseSeries(version) {
  const [major, minor] = normalizeVersion(version).split('.');
  return `${major}.${minor}.x`;
}

export function releaseAssetName(version) {
  return `SkillBox_${normalizeVersion(version)}_universal.dmg`;
}

export function updaterBundleAssetName(version) {
  return `SkillBox_${normalizeVersion(version)}_universal.app.tar.gz`;
}

export function updaterSignatureAssetName(version) {
  return `${updaterBundleAssetName(version)}.sig`;
}

export function buildLatestJson({ version, notes, pubDate, url, signature }) {
  const releaseVersion = normalizeVersion(version);
  const normalizedUrl = String(url || '').trim();
  const normalizedSignature = String(signature || '').trim();
  if (!normalizedUrl.startsWith('https://')) {
    throw new Error(`Updater URL must be HTTPS: ${url || ''}`);
  }
  if (!normalizedSignature) {
    throw new Error('Updater signature must not be empty.');
  }

  return {
    version: releaseVersion,
    notes: String(notes || '').trim(),
    pub_date: String(pubDate || '').trim(),
    platforms: {
      'darwin-aarch64': {
        signature: normalizedSignature,
        url: normalizedUrl
      },
      'darwin-x86_64': {
        signature: normalizedSignature,
        url: normalizedUrl
      }
    }
  };
}

export function assertReleaseAssets(release, version) {
  const releaseVersion = normalizeVersion(version);
  const assets = release?.assets || [];
  for (const expectedName of [
    releaseAssetName(releaseVersion),
    updaterBundleAssetName(releaseVersion),
    updaterSignatureAssetName(releaseVersion),
    'latest.json'
  ]) {
    const asset = assets.find((candidate) => candidate.name === expectedName);
    if (!asset) {
      throw new Error(`Release asset ${expectedName} is missing.`);
    }
    if (!asset.digest) {
      throw new Error(`Release asset ${expectedName} is missing a digest.`);
    }
  }
}

export function extractChangelogEntry(content, version) {
  const lines = String(content).split('\n');
  const header = `## ${normalizeVersion(version)}`;
  const start = lines.findIndex((line) => line.trim() === header);
  if (start === -1) {
    return '';
  }
  const body = [];
  for (const line of lines.slice(start + 1)) {
    if (/^##\s+/.test(line)) break;
    body.push(line);
  }
  return body.join('\n').trim();
}

export function insertChangelogEntry(content, version, notes) {
  const releaseVersion = normalizeVersion(version);
  const normalizedNotes = normalizeReleaseNotes(notes);
  if (extractChangelogEntry(content, releaseVersion)) {
    throw new Error(`CHANGELOG.md already has a ${releaseVersion} entry.`);
  }

  const marker = '## Unreleased\n\n';
  const markerIndex = content.indexOf(marker);
  if (markerIndex === -1) {
    throw new Error('CHANGELOG.md is missing the "## Unreleased" section.');
  }

  const bodyStart = markerIndex + marker.length;
  const nextHeader = content.indexOf('\n## ', bodyStart);
  if (nextHeader === -1) {
    throw new Error('CHANGELOG.md is missing a release section after "## Unreleased".');
  }

  const unreleased = content.slice(bodyStart, nextHeader).trim();
  if (unreleased && unreleased !== '- No unreleased changes.') {
    throw new Error('Move existing Unreleased notes into the release notes file before running release automation.');
  }

  return `${content.slice(0, bodyStart)}- No unreleased changes.\n\n## ${releaseVersion}\n\n${normalizedNotes}\n${content.slice(nextHeader + 1)}`;
}

export function updateSecuritySupport(content, version) {
  const series = releaseSeries(version);
  if (content.includes(`| \`${series}\` | Yes |`)) {
    return content;
  }

  const updated = content.replace(
    /^\| `(\d+\.\d+\.x)` \| Yes \|$/m,
    (_match, previousSeries) => `| \`${series}\` | Yes |\n| \`${previousSeries}\` | No |`
  );
  if (updated === content) {
    throw new Error('SECURITY.md does not contain a supported release line to update.');
  }
  return updated;
}

export function updateCaskContent(content, version, sha256) {
  const releaseVersion = normalizeVersion(version);
  const digest = String(sha256 || '').trim().replace(/^sha256:/, '');
  if (!/^[0-9a-f]{64}$/i.test(digest)) {
    throw new Error(`Invalid SHA-256 digest: ${sha256 || ''}`);
  }

  return content
    .replace(/version "[^"]+"/, `version "${releaseVersion}"`)
    .replace(/sha256 "[^"]+"/, `sha256 "${digest.toLowerCase()}"`)
    .replace(/github\.com\/santosli\/skill-box/g, `github.com/${REPOSITORY}`)
    .replace(/github\.com\/santosli\/SkillBox/g, `github.com/${REPOSITORY}`);
}

export function updateIssueTemplateVersionPlaceholder(content, version) {
  const releaseVersion = normalizeVersion(version);
  const updated = content.replace(
    /(label: SkillBox version\n\s+placeholder: ")[^"]+(")/,
    `$1${releaseVersion}$2`
  );
  if (updated === content) {
    throw new Error('Issue template is missing a SkillBox version placeholder.');
  }
  return updated;
}

export function updateReadmeReleaseAssets(content, version) {
  const assetName = releaseAssetName(version);
  const checksumPattern = /SkillBox_[0-9A-Za-z.-]+_universal\.dmg\.sha256/g;
  const dmgPattern = /SkillBox_[0-9A-Za-z.-]+_universal\.dmg(?!\.sha256)/g;
  if (!checksumPattern.test(content)) {
    throw new Error('README release assets are missing a DMG checksum filename.');
  }
  if (!dmgPattern.test(content)) {
    throw new Error('README release assets are missing a DMG filename.');
  }
  return content
    .replace(checksumPattern, `${assetName}.sha256`)
    .replace(dmgPattern, assetName);
}

function normalizeReleaseNotes(notes) {
  const normalized = String(notes || '').replace(/\r\n/g, '\n').trim();
  if (!normalized) {
    throw new Error('Release notes must not be empty.');
  }
  if (/^##\s+/m.test(normalized)) {
    throw new Error('Release notes file should contain bullet points, not a changelog heading.');
  }
  return `${normalized}\n`;
}

function run(command, args, options = {}) {
  console.log(`$ ${[command, ...args].join(' ')}`);
  const result = spawnSync(command, args, {
    cwd: process.cwd(),
    stdio: 'inherit',
    ...options
  });
  if (result.status !== 0) {
    throw new Error(`Command failed: ${command} ${args.join(' ')}`);
  }
}

function capture(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: process.cwd(),
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    ...options
  });
  if (result.status !== 0) {
    throw new Error((result.stderr || result.stdout || `Command failed: ${command} ${args.join(' ')}`).trim());
  }
  return result.stdout.trim();
}

function readText(file) {
  return readFileSync(file, 'utf8');
}

function writeText(file, content) {
  writeFileSync(file, content);
}

function updateJsonVersion(file, version, update) {
  const data = JSON.parse(readText(file));
  update(data, version);
  writeText(file, `${JSON.stringify(data, null, 2)}\n`);
}

function replaceInFile(file, pattern, replacement) {
  const before = readText(file);
  const after = before.replace(pattern, replacement);
  if (after === before) {
    throw new Error(`${file} did not match ${pattern}`);
  }
  writeText(file, after);
}

function prepareRelease(version, notesFile) {
  const releaseVersion = normalizeVersion(version);
  const notes = readText(notesFile);
  const assetName = releaseAssetName(releaseVersion);

  updateJsonVersion('package.json', releaseVersion, (data, value) => {
    data.version = value;
  });
  updateJsonVersion('apps/desktop/package.json', releaseVersion, (data, value) => {
    data.version = value;
  });
  updateJsonVersion('package-lock.json', releaseVersion, (data, value) => {
    data.version = value;
    data.packages[''].version = value;
    data.packages['apps/desktop'].version = value;
  });
  updateJsonVersion('apps/desktop/src-tauri/tauri.conf.json', releaseVersion, (data, value) => {
    data.version = value;
  });

  for (const file of [
    'apps/desktop/src-tauri/Cargo.toml',
    'crates/skillbox-cli/Cargo.toml',
    'crates/skillbox-core/Cargo.toml',
    'crates/skillbox-git/Cargo.toml',
    'crates/skillbox-github/Cargo.toml'
  ]) {
    replaceInFile(file, /^version = "[^"]+"/m, `version = "${releaseVersion}"`);
  }

  writeText('CHANGELOG.md', insertChangelogEntry(readText('CHANGELOG.md'), releaseVersion, notes));
  writeText('SECURITY.md', updateSecuritySupport(readText('SECURITY.md'), releaseVersion));

  replaceInFile('README.md', /Current release: `v[^`]+`/, `Current release: \`v${releaseVersion}\``);
  writeText('README.md', updateReadmeReleaseAssets(readText('README.md'), releaseVersion));

  replaceInFile('README.zh-CN.md', /当前版本：`v[^`]+`/, `当前版本：\`v${releaseVersion}\``);
  writeText('README.zh-CN.md', updateReadmeReleaseAssets(readText('README.zh-CN.md'), releaseVersion));

  replaceInFile('docs/release.md', /Current tag: `v[^`]+`/, `Current tag: \`v${releaseVersion}\``);
  replaceInFile('docs/release.md', /Current DMG asset: `SkillBox_[^`]+_universal\.dmg`/, `Current DMG asset: \`${assetName}\``);
  replaceInFile(
    'docs/release.md',
    /Current updater asset: `SkillBox_[^`]+_universal\.app\.tar\.gz`/,
    `Current updater asset: \`${updaterBundleAssetName(releaseVersion)}\``
  );
  replaceInFile(
    'docs/release.md',
    /Current updater signature: `SkillBox_[^`]+_universal\.app\.tar\.gz\.sig`/,
    `Current updater signature: \`${updaterSignatureAssetName(releaseVersion)}\``
  );
  replaceInFile('docs/release.md', /Current checksum asset: `SkillBox_[^`]+_universal\.dmg\.sha256`/, `Current checksum asset: \`${assetName}.sha256\``);
  replaceInFile('docs/release.md', /git tag v[0-9A-Za-z.-]+/, `git tag v${releaseVersion}`);
  replaceInFile('docs/release.md', /git push origin v[0-9A-Za-z.-]+/, `git push origin v${releaseVersion}`);

  replaceInFile('docs/roadmap.md', /Current Focus: \d+\.\d+\.x/, `Current Focus: ${releaseSeries(releaseVersion)}`);
  writeText(
    '.github/ISSUE_TEMPLATE/bug_report.yml',
    updateIssueTemplateVersionPlaceholder(readText('.github/ISSUE_TEMPLATE/bug_report.yml'), releaseVersion)
  );
  writeText(
    '.github/ISSUE_TEMPLATE/installation_problem.yml',
    updateIssueTemplateVersionPlaceholder(readText('.github/ISSUE_TEMPLATE/installation_problem.yml'), releaseVersion)
  );

  run('cargo', ['metadata', '--offline', '--format-version', '1'], { stdio: 'ignore' });
}

function currentBranch() {
  return capture('git', ['branch', '--show-current']);
}

function assertOnMain() {
  const branch = currentBranch();
  if (branch !== 'main') {
    throw new Error(`Release automation must run on main, found ${branch || '(detached)'}.`);
  }
}

function assertClean() {
  const status = gitStatusPorcelain();
  if (status) {
    throw new Error(`Working tree must be clean before this step:\n${status}`);
  }
}

function gitStatusPorcelain() {
  return capture('git', ['status', '--porcelain']);
}

function changedFiles() {
  const status = gitStatusPorcelain();
  if (!status) return [];
  return status
    .split('\n')
    .map((line) => line.slice(3).trim())
    .filter(Boolean);
}

function assertOnlyReleasePrepChanges() {
  const allowed = new Set(VERSION_FILES);
  const unexpected = changedFiles().filter((file) => !allowed.has(file));
  if (unexpected.length) {
    throw new Error(`Release publish found unrelated dirty files:\n${unexpected.join('\n')}`);
  }
}

function ensureVersionPrepared(version) {
  const releaseVersion = normalizeVersion(version);
  const rootPackage = JSON.parse(readText('package.json'));
  const desktopPackage = JSON.parse(readText('apps/desktop/package.json'));
  const tauriConfig = JSON.parse(readText('apps/desktop/src-tauri/tauri.conf.json'));
  for (const [name, value] of [
    ['package.json', rootPackage.version],
    ['apps/desktop/package.json', desktopPackage.version],
    ['tauri.conf.json', tauriConfig.version]
  ]) {
    if (value !== releaseVersion) {
      throw new Error(`${name} version is ${value}, expected ${releaseVersion}.`);
    }
  }
  if (!extractChangelogEntry(readText('CHANGELOG.md'), releaseVersion)) {
    throw new Error(`CHANGELOG.md is missing a ${releaseVersion} entry.`);
  }
}

function runChecks() {
  for (const [command, args] of CHECKS) {
    run(command, args);
  }
}

function commitReleasePrep(version) {
  run('git', ['add', ...VERSION_FILES]);
  run('git', [
    'commit',
    '-m',
    `chore(github): prepare v${version} release`,
    '-m',
    `- Bump SkillBox package, Rust crate, Tauri, and lockfile versions to ${version}.`,
    '-m',
    '- Add the changelog entry and update public release/security docs.',
    '-m',
    '- Verification: npm run release automation checks.'
  ]);
}

function commitCask(version, sha256) {
  const cask = updateCaskContent(readText(CASK_PATH), version, sha256);
  writeText(CASK_PATH, cask);
  run('ruby', ['-c', CASK_PATH]);
  run('git', ['diff', '--check']);
  run('git', ['add', CASK_PATH]);
  run('git', [
    'commit',
    '-m',
    `chore(github): update v${version} cask checksum`,
    '-m',
    '- Update the SkillBox cask template to the published DMG asset.',
    '-m',
    '- Verification: ruby -c packaging/homebrew/Casks/skillbox.rb; git diff --check.'
  ]);
  run('git', ['push', 'origin', 'main']);
}

async function runReleaseWorkflow({ branch, event, headSha }) {
  await sleep(5000);
  const runId = latestWorkflowRunId({ branch, event, headSha });
  run('gh', ['run', 'watch', String(runId), '--exit-status']);
  return runId;
}

function latestWorkflowRunId({ branch, event, headSha }) {
  const runs = JSON.parse(
    capture('gh', [
      'run',
      'list',
      '--workflow',
      RELEASE_WORKFLOW,
      '--branch',
      branch,
      '--event',
      event,
      '--limit',
      '10',
      '--json',
      'databaseId,headSha,createdAt'
    ])
  );
  const run = headSha ? runs.find((candidate) => candidate.headSha === headSha) : runs[0];
  if (!run) {
    throw new Error(`Could not find ${RELEASE_WORKFLOW} run for ${branch}/${event}.`);
  }
  return run.databaseId;
}

function releaseDmgSha(version) {
  const assetName = releaseAssetName(version);
  const release = JSON.parse(capture('gh', ['release', 'view', `v${version}`, '--json', 'assets']));
  assertReleaseAssets(release, version);
  const asset = release.assets.find((candidate) => candidate.name === assetName);
  if (!asset?.digest) {
    throw new Error(`Release asset ${assetName} is missing a digest.`);
  }
  return asset.digest.replace(/^sha256:/, '');
}

function updateTap({ version, sha256, tapDir }) {
  const workingDir = tapDir || mkdtempSync(path.join(tmpdir(), `skillbox-homebrew-tap-${version}-`));
  if (!existsSync(workingDir)) {
    run('git', ['clone', TAP_REPOSITORY, workingDir]);
  } else if (!existsSync(path.join(workingDir, '.git'))) {
    run('git', ['clone', TAP_REPOSITORY, workingDir]);
  }

  const previousCwd = process.cwd();
  process.chdir(workingDir);
  try {
    run('git', ['pull', '--ff-only', 'origin', 'main']);
    const tapCask = 'Casks/skillbox.rb';
    writeText(tapCask, updateCaskContent(readText(tapCask), version, sha256));
    run('ruby', ['-c', tapCask]);
    run('git', ['diff', '--check']);
    run('git', ['add', tapCask]);
    run('git', [
      'commit',
      '-m',
      `chore(cask): update skillbox to ${version}`,
      '-m',
      '- Update the SkillBox cask version, checksum, and canonical repository URLs.',
      '-m',
      '- Verification: ruby -c Casks/skillbox.rb; git diff --check.'
    ]);
    run('git', ['push', 'origin', 'main']);
  } finally {
    process.chdir(previousCwd);
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function publishRelease(version, options) {
  const releaseVersion = normalizeVersion(version);
  assertOnMain();
  assertClean();
  ensureVersionPrepared(releaseVersion);

  if (!options.skipChecks) {
    runChecks();
  }

  run('git', ['fetch', 'origin', 'main']);
  run('git', ['pull', '--ff-only', 'origin', 'main']);
  const headSha = capture('git', ['rev-parse', 'HEAD']);
  run('git', ['push', 'origin', 'main']);

  run('gh', ['workflow', 'run', RELEASE_WORKFLOW, '--ref', 'main']);
  await runReleaseWorkflow({ branch: 'main', event: 'workflow_dispatch', headSha });

  const tag = `v${releaseVersion}`;
  if (capture('git', ['tag', '--list', tag])) {
    throw new Error(`Tag already exists locally: ${tag}`);
  }
  run('git', ['tag', tag]);
  run('git', ['push', 'origin', tag]);
  await runReleaseWorkflow({ branch: tag, event: 'push', headSha });

  const sha256 = releaseDmgSha(releaseVersion);
  commitCask(releaseVersion, sha256);

  if (!options.skipTap) {
    updateTap({ version: releaseVersion, sha256, tapDir: options.tapDir });
  }
}

function usage() {
  return `Usage:
  npm run release -- <version> --notes-file <file> --yes [--tap-dir <dir>]
  npm run release:prepare -- <version> --notes-file <file>
  npm run release:publish -- <version> --yes [--tap-dir <dir>]

Options:
  --notes-file <file>  Bullet list inserted into CHANGELOG.md for the release.
  --tap-dir <dir>      Existing or temporary checkout for santosli/homebrew-tap.
  --skip-checks        Skip local validation commands during publish.
  --skip-tap           Do not update santosli/homebrew-tap after release.
  --yes                Required before commands that push, tag, or publish.
`;
}

async function main(argv = process.argv.slice(2)) {
  const parsed = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      'notes-file': { type: 'string' },
      'tap-dir': { type: 'string' },
      'skip-checks': { type: 'boolean', default: false },
      'skip-tap': { type: 'boolean', default: false },
      yes: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false }
    }
  });

  if (parsed.values.help) {
    console.log(usage());
    return;
  }

  const positionals = [...parsed.positionals];
  const command = ['prepare', 'publish', 'full'].includes(positionals[0]) ? positionals.shift() : 'full';
  const version = normalizeVersion(positionals.shift());
  if (positionals.length) {
    throw new Error(`Unexpected arguments: ${positionals.join(' ')}`);
  }

  if (command !== 'publish' && !parsed.values['notes-file']) {
    throw new Error('--notes-file is required for prepare/full releases.');
  }
  if (command !== 'prepare' && !parsed.values.yes) {
    throw new Error('--yes is required before pushing, tagging, or publishing a release.');
  }

  assertOnMain();

  if (command === 'full' || command === 'prepare') {
    assertClean();
  }

  if (command === 'prepare' || command === 'full') {
    prepareRelease(version, parsed.values['notes-file']);
  }

  if (command === 'prepare') {
    console.log(`Prepared v${version}. Review the diff, then run: npm run release:publish -- ${version} --yes`);
    return;
  }

  if (command === 'full') {
    runChecks();
    commitReleasePrep(version);
  }

  let checksAlreadyRun = command === 'full';
  if (command === 'publish' && gitStatusPorcelain()) {
    assertOnlyReleasePrepChanges();
    ensureVersionPrepared(version);
    if (!parsed.values['skip-checks']) {
      runChecks();
      checksAlreadyRun = true;
    }
    commitReleasePrep(version);
  }

  await publishRelease(version, {
    tapDir: parsed.values['tap-dir'],
    skipChecks: checksAlreadyRun ? true : parsed.values['skip-checks'],
    skipTap: parsed.values['skip-tap']
  });
}

const executedPath = process.argv[1] ? path.resolve(process.argv[1]) : '';
if (fileURLToPath(import.meta.url) === executedPath) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
