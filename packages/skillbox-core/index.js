import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(import.meta.url);

export const VERSION = '0.1.0';

export function expandHome(input) {
  if (!input || input === '~') return os.homedir();
  if (input.startsWith('~/')) return path.join(os.homedir(), input.slice(2));
  return input;
}

export function defaultManagedRoot() {
  return expandHome(process.env.SKILLBOX_HOME || '~/.skillbox');
}

export function defaultRuntimeRoots() {
  return [expandHome('~/.codex/skills'), expandHome('~/.agents/skills'), expandHome('~/.claude/skills')];
}

export function managedPaths(managedRoot = defaultManagedRoot()) {
  const root = path.resolve(expandHome(managedRoot));
  return {
    root,
    userSkillsRoot: path.join(root, 'user-skills'),
    remoteSkillsRoot: path.join(root, 'remote-skills'),
    databasePath: path.join(root, 'skillbox.sqlite')
  };
}

export function ensureManagedLayout(managedRoot = defaultManagedRoot()) {
  const paths = managedPaths(managedRoot);
  fs.mkdirSync(paths.userSkillsRoot, { recursive: true });
  fs.mkdirSync(paths.remoteSkillsRoot, { recursive: true });
  initDatabase(paths.databasePath).close();
  return paths;
}

export function parseSkillMarkdown(content) {
  const result = { frontmatter: {}, body: content };
  const lines = content.split(/\r?\n/);
  if (lines[0] !== '---') return result;

  const end = lines.findIndex((line, index) => index > 0 && line === '---');
  if (end === -1) return result;

  const frontmatter = {};
  for (const line of lines.slice(1, end)) {
    if (!line.trim() || /^\s/.test(line)) continue;
    const match = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!match) continue;
    const [, key, rawValue] = match;
    frontmatter[key] = unquoteYamlScalar(rawValue.trim());
  }

  return {
    frontmatter,
    body: lines.slice(end + 1).join('\n')
  };
}

export function readSkill(skillDir) {
  const skillPath = path.resolve(expandHome(skillDir));
  const skillMdPath = path.join(skillPath, 'SKILL.md');
  if (!fs.existsSync(skillMdPath)) {
    throw new Error(`SKILL.md not found in ${skillPath}`);
  }

  const content = fs.readFileSync(skillMdPath, 'utf8');
  const parsed = parseSkillMarkdown(content);
  const name = parsed.frontmatter.name || path.basename(skillPath);
  const description = parsed.frontmatter.description || '';
  const version = parsed.frontmatter.version || '';

  return {
    name,
    description,
    version,
    path: skillPath,
    skillMdPath,
    contentHash: sha256(content),
    frontmatter: parsed.frontmatter,
    body: parsed.body
  };
}

export function scanSkillRoots(roots = defaultRuntimeRoots(), options = {}) {
  const maxDepth = options.maxDepth ?? 3;
  const includeBody = options.includeBody ?? false;
  const includeFrontmatter = options.includeFrontmatter ?? false;
  const skills = [];
  const errors = [];

  for (const rootInput of roots) {
    const root = path.resolve(expandHome(rootInput));
    if (!fs.existsSync(root)) continue;

    try {
      for (const skillDir of findSkillDirs(root, maxDepth)) {
        try {
          const skill = readSkill(skillDir);
          const { body, frontmatter, ...summary } = skill;
          skills.push({
            ...summary,
            ...(includeBody ? { body } : {}),
            ...(includeFrontmatter ? { frontmatter: skill.frontmatter } : {}),
            sourceRoot: root,
            isSymlink: fs.lstatSync(skillDir).isSymbolicLink(),
            realPath: fs.realpathSync(skillDir)
          });
        } catch (error) {
          errors.push({ root, path: skillDir, error: error.message });
        }
      }
    } catch (error) {
      errors.push({ root, error: error.message });
    }
  }

  skills.sort((a, b) => a.name.localeCompare(b.name));
  return { roots: roots.map((root) => path.resolve(expandHome(root))), skills, errors };
}

export function initDatabase(databasePath) {
  fs.mkdirSync(path.dirname(databasePath), { recursive: true });
  const { DatabaseSync } = require('node:sqlite');
  const db = new DatabaseSync(databasePath);
  db.exec(`
    CREATE TABLE IF NOT EXISTS skills (
      name TEXT PRIMARY KEY,
      type TEXT NOT NULL,
      description TEXT NOT NULL DEFAULT '',
      version TEXT NOT NULL DEFAULT '',
      managed_path TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'ok',
      content_hash TEXT NOT NULL DEFAULT '',
      source_json TEXT NOT NULL DEFAULT '{}',
      updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS deployments (
      skill_name TEXT NOT NULL,
      target_root TEXT NOT NULL,
      target_path TEXT NOT NULL,
      mode TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      PRIMARY KEY (skill_name, target_root)
    );

    CREATE TABLE IF NOT EXISTS operations (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      type TEXT NOT NULL,
      skill_name TEXT,
      status TEXT NOT NULL,
      message TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL
    );
  `);
  return db;
}

export function indexSkills(skills, managedRoot = defaultManagedRoot()) {
  const paths = ensureManagedLayout(managedRoot);
  const db = initDatabase(paths.databasePath);
  const insert = db.prepare(`
    INSERT INTO skills (
      name, type, description, version, managed_path, status, content_hash, source_json, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(name) DO UPDATE SET
      description = excluded.description,
      version = excluded.version,
      managed_path = excluded.managed_path,
      status = excluded.status,
      content_hash = excluded.content_hash,
      source_json = excluded.source_json,
      updated_at = excluded.updated_at
  `);

  const now = new Date().toISOString();
  for (const skill of skills) {
    insert.run(
      skill.name,
      skill.type || 'discovered',
      skill.description || '',
      skill.version || '',
      skill.managedPath || skill.path,
      skill.status || 'ok',
      skill.contentHash || '',
      JSON.stringify(skill.source || {}),
      now
    );
  }
  db.close();
  return { databasePath: paths.databasePath, indexed: skills.length };
}

export function importSkill({ sourceDir, type = 'user', managedRoot = defaultManagedRoot() }) {
  if (!['user', 'remote'].includes(type)) {
    throw new Error(`Unsupported skill type: ${type}`);
  }

  const paths = ensureManagedLayout(managedRoot);
  const skill = readSkill(sourceDir);
  const safeName = validateSkillName(skill.name);
  let managedPath;
  let source;

  if (type === 'user') {
    managedPath = path.join(paths.userSkillsRoot, safeName);
    copySkillDirectory(skill.path, managedPath);
    source = { type: 'local' };
  } else {
    const versionId = `manual-${skill.contentHash.slice(0, 12)}`;
    const remoteRoot = path.join(paths.remoteSkillsRoot, safeName);
    managedPath = path.join(remoteRoot, 'versions', versionId);
    copySkillDirectory(skill.path, managedPath);
    updateCurrentSymlink(remoteRoot, managedPath);
    writeJson(path.join(remoteRoot, 'source.json'), {
      type: 'manual',
      installedSha: versionId,
      installedAt: new Date().toISOString()
    });
    source = { type: 'manual', installedSha: versionId };
  }

  const record = {
    ...skill,
    type,
    managedPath,
    source
  };
  indexSkills([record], managedRoot);
  logOperation(paths.databasePath, 'import', skill.name, 'ok', `Imported ${skill.name} as ${type}`);
  return record;
}

export function deploySkill({ skillName, managedRoot = defaultManagedRoot(), targetRoot }) {
  if (!targetRoot) throw new Error('Missing targetRoot');
  const paths = ensureManagedLayout(managedRoot);
  const safeName = validateSkillName(skillName);
  const managedPath = resolveManagedSkillPath(paths, safeName);
  const targetRootPath = path.resolve(expandHome(targetRoot));
  const targetPath = path.join(targetRootPath, safeName);

  fs.mkdirSync(targetRootPath, { recursive: true });
  let shouldCreateSymlink = false;
  if (fs.existsSync(targetPath) || fs.lstatSync(targetPath, { throwIfNoEntry: false })) {
    const stat = fs.lstatSync(targetPath);
    if (!stat.isSymbolicLink()) {
      throw new Error(`Refusing to overwrite existing non-symlink target: ${targetPath}`);
    }
    const linkedPath = path.resolve(path.dirname(targetPath), fs.readlinkSync(targetPath));
    if (fs.realpathSync(linkedPath) !== fs.realpathSync(managedPath)) {
      throw new Error(`Refusing to replace symlink pointing elsewhere: ${targetPath}`);
    }
    if (linkedPath !== managedPath) {
      fs.unlinkSync(targetPath);
      shouldCreateSymlink = true;
    }
  } else {
    shouldCreateSymlink = true;
  }

  if (shouldCreateSymlink) {
    fs.symlinkSync(managedPath, targetPath, 'dir');
  }

  const db = initDatabase(paths.databasePath);
  db.prepare(`
    INSERT INTO deployments (skill_name, target_root, target_path, mode, updated_at)
    VALUES (?, ?, ?, 'symlink', ?)
    ON CONFLICT(skill_name, target_root) DO UPDATE SET
      target_path = excluded.target_path,
      mode = excluded.mode,
      updated_at = excluded.updated_at
  `).run(safeName, targetRootPath, targetPath, new Date().toISOString());
  db.close();
  logOperation(paths.databasePath, 'deploy', safeName, 'ok', `Deployed to ${targetPath}`);

  return { skillName: safeName, managedPath, targetRoot: targetRootPath, targetPath, mode: 'symlink' };
}

export function parseGitHubSkillUrl(input) {
  rejectRawUrlPathTraversal(input);
  const url = new URL(input);
  let owner;
  let repo;
  let ref = 'main';
  let skillPath = '';
  let kind = 'github';

  if (url.hostname === 'github.com') {
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts.length < 2) throw new Error('GitHub URL must include owner and repo');
    owner = parts[0];
    repo = trimGitSuffix(parts[1]);

    if (parts[2] === 'tree' || parts[2] === 'blob') {
      kind = parts[2];
      const parsed = splitRefAndSkillPath(parts.slice(3));
      ref = parsed.ref;
      skillPath = stripSkillMd(parsed.path);
    } else if (parts.length > 2) {
      skillPath = parts.slice(2).join('/');
    }
  } else if (url.hostname === 'raw.githubusercontent.com') {
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts.length < 4) throw new Error('Raw GitHub URL must include owner, repo, ref, and path');
    kind = 'raw';
    owner = parts[0];
    repo = trimGitSuffix(parts[1]);
    const parsed = splitRefAndSkillPath(parts.slice(2));
    ref = parsed.ref;
    skillPath = stripSkillMd(parsed.path);
  } else if (url.hostname === 'api.github.com') {
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts[0] !== 'repos' || parts[3] !== 'contents') {
      throw new Error('Unsupported GitHub API URL');
    }
    kind = 'api';
    owner = parts[1];
    repo = trimGitSuffix(parts[2]);
    ref = url.searchParams.get('ref') || ref;
    skillPath = stripSkillMd(parts.slice(4).join('/'));
  } else {
    throw new Error('Only GitHub URLs are supported');
  }

  if (!owner || !repo || !skillPath) {
    throw new Error('GitHub URL must point to a skill directory or SKILL.md file');
  }
  validateRepoRelativePath(skillPath);
  validateGitReference(ref);

  return {
    owner,
    repo,
    ref,
    path: skillPath,
    url: `https://github.com/${owner}/${repo}/tree/${ref}/${skillPath}`,
    repoUrl: `https://github.com/${owner}/${repo}.git`,
    kind
  };
}

export function installRemoteSkillFromGitHub({
  url,
  managedRoot = defaultManagedRoot(),
  targetRoot
}) {
  const source = parseGitHubSkillUrl(url);
  const paths = ensureManagedLayout(managedRoot);
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'skillbox-install-'));
  const repoDir = path.join(tempRoot, 'repo');

  try {
    execFileSync('git', [
      'clone',
      '--depth',
      '1',
      '--filter=blob:none',
      '--sparse',
      '--branch',
      source.ref,
      '--',
      source.repoUrl,
      repoDir
    ], { stdio: 'pipe' });
    execFileSync('git', ['-C', repoDir, 'sparse-checkout', 'set', source.path], { stdio: 'pipe' });
    const installedSha = execFileSync('git', ['-C', repoDir, 'rev-parse', 'HEAD'], {
      encoding: 'utf8'
    }).trim();

    const skillSourcePath = path.join(repoDir, source.path);
    const skill = readSkill(skillSourcePath);
    const safeName = validateSkillName(skill.name);
    const remoteRoot = path.join(paths.remoteSkillsRoot, safeName);
    const versionPath = path.join(remoteRoot, 'versions', installedSha);

    if (!fs.existsSync(versionPath)) {
      copySkillDirectory(skillSourcePath, versionPath);
    }
    updateCurrentSymlink(remoteRoot, versionPath);
    writeJson(path.join(remoteRoot, 'source.json'), {
      ...source,
      type: 'github',
      installedSha,
      latestSha: installedSha,
      installedAt: new Date().toISOString()
    });

    const record = {
      ...skill,
      type: 'remote',
      managedPath: versionPath,
      source: { ...source, type: 'github', installedSha, latestSha: installedSha }
    };
    indexSkills([record], managedRoot);
    logOperation(paths.databasePath, 'install', safeName, 'ok', `Installed ${source.url}`);

    const deployment = targetRoot ? deploySkill({ skillName: safeName, managedRoot, targetRoot }) : null;
    return { skill: record, source, installedSha, versionPath, deployment };
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

export function checkRemoteUpdates({ managedRoot = defaultManagedRoot(), skillName } = {}) {
  const paths = ensureManagedLayout(managedRoot);
  const remoteRoots = fs.existsSync(paths.remoteSkillsRoot)
    ? fs.readdirSync(paths.remoteSkillsRoot, { withFileTypes: true }).filter((entry) => entry.isDirectory())
    : [];
  const updates = [];

  for (const entry of remoteRoots) {
    if (skillName && entry.name !== skillName) continue;
    const sourcePath = path.join(paths.remoteSkillsRoot, entry.name, 'source.json');
    if (!fs.existsSync(sourcePath)) continue;
    const source = readJson(sourcePath);
    validateRemoteSource(source);
    if (source.type !== 'github') continue;

    const remoteLine = execFileSync('git', ['ls-remote', '--', source.repoUrl, source.ref], {
      encoding: 'utf8'
    }).trim();
    const latestSha = remoteLine.split(/\s+/)[0] || '';
    updates.push({
      skillName: entry.name,
      installedSha: source.installedSha,
      latestSha,
      updateAvailable: Boolean(latestSha && latestSha !== source.installedSha)
    });
  }

  return updates;
}

export function rollbackRemoteSkill({ skillName, toSha, managedRoot = defaultManagedRoot() }) {
  if (!toSha) throw new Error('Missing rollback SHA');
  const paths = ensureManagedLayout(managedRoot);
  const safeName = validateSkillName(skillName);
  const remoteRoot = path.join(paths.remoteSkillsRoot, safeName);
  const versionsRoot = path.join(remoteRoot, 'versions');
  const versions = fs.readdirSync(versionsRoot);
  const exact = versions.find((version) => version === toSha);
  const prefixMatches = exact ? [] : versions.filter((version) => version.startsWith(toSha));
  if (prefixMatches.length > 1) throw new Error(`Version prefix is ambiguous: ${toSha}`);
  const match = exact || prefixMatches[0];
  if (!match) throw new Error(`No version found for ${safeName}: ${toSha}`);
  const versionPath = path.join(versionsRoot, match);
  updateCurrentSymlink(remoteRoot, versionPath);
  logOperation(paths.databasePath, 'rollback', safeName, 'ok', `Rolled back to ${match}`);
  return { skillName: safeName, version: match, currentPath: path.join(remoteRoot, 'current') };
}

export function gitStatus(repoPath) {
  const root = path.resolve(expandHome(repoPath));
  if (!fs.existsSync(path.join(root, '.git'))) return { initialized: false, root };
  const branch = execFileSync('git', ['-C', root, 'branch', '--show-current'], { encoding: 'utf8' }).trim();
  const status = execFileSync('git', ['-C', root, 'status', '--short', '--branch'], {
    encoding: 'utf8'
  }).trim();
  return { initialized: true, root, branch, status, dirty: status.split('\n').some((line) => !line.startsWith('##')) };
}

export function syncUserSkills({ managedRoot = defaultManagedRoot(), remote, commitMessage, push = false } = {}) {
  const paths = ensureManagedLayout(managedRoot);
  if (!fs.existsSync(path.join(paths.userSkillsRoot, '.git'))) {
    execFileSync('git', ['-C', paths.userSkillsRoot, 'init', '-b', 'main'], { stdio: 'pipe' });
  }
  if (remote) {
    const remotes = execFileSync('git', ['-C', paths.userSkillsRoot, 'remote'], { encoding: 'utf8' })
      .split(/\s+/)
      .filter(Boolean);
    if (remotes.includes('origin')) {
      execFileSync('git', ['-C', paths.userSkillsRoot, 'remote', 'set-url', 'origin', remote], { stdio: 'pipe' });
    } else {
      execFileSync('git', ['-C', paths.userSkillsRoot, 'remote', 'add', 'origin', remote], { stdio: 'pipe' });
    }
  }

  execFileSync('git', ['-C', paths.userSkillsRoot, 'add', '.'], { stdio: 'pipe' });
  const porcelain = execFileSync('git', ['-C', paths.userSkillsRoot, 'status', '--porcelain'], {
    encoding: 'utf8'
  }).trim();
  let committed = false;
  if (porcelain && commitMessage) {
    execFileSync('git', ['-C', paths.userSkillsRoot, 'commit', '-m', commitMessage], { stdio: 'pipe' });
    committed = true;
  }
  if (push) {
    execFileSync('git', ['-C', paths.userSkillsRoot, 'push', '-u', 'origin', 'main'], { stdio: 'pipe' });
  }

  return { ...gitStatus(paths.userSkillsRoot), committed, pushed: push };
}

function findSkillDirs(root, maxDepth) {
  const found = [];

  function visit(current, depth) {
    if (depth > maxDepth) return;
    const skillMdPath = path.join(current, 'SKILL.md');
    if (fs.existsSync(skillMdPath)) {
      found.push(current);
      return;
    }
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (!entry.isDirectory() || entry.name.startsWith('.')) continue;
      visit(path.join(current, entry.name), depth + 1);
    }
  }

  visit(root, 0);
  return found;
}

function resolveManagedSkillPath(paths, skillName) {
  const userPath = path.join(paths.userSkillsRoot, skillName);
  if (fs.existsSync(path.join(userPath, 'SKILL.md'))) return userPath;

  const remoteCurrent = path.join(paths.remoteSkillsRoot, skillName, 'current');
  if (fs.existsSync(path.join(remoteCurrent, 'SKILL.md'))) return remoteCurrent;

  throw new Error(`Managed skill not found: ${skillName}`);
}

function copySkillDirectory(source, destination) {
  if (fs.existsSync(destination)) {
    throw new Error(`Destination already exists: ${destination}`);
  }
  const sourceRoot = fs.realpathSync(source);
  try {
    copyRecursively(source, destination, sourceRoot);
  } catch (error) {
    fs.rmSync(destination, { recursive: true, force: true });
    throw error;
  }
}

function copyRecursively(source, destination, sourceRoot) {
  const stat = fs.lstatSync(source);
  if (stat.isDirectory()) {
    fs.mkdirSync(destination, { recursive: true });
    for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
      if (entry.name === '.git') continue;
      copyRecursively(path.join(source, entry.name), path.join(destination, entry.name), sourceRoot);
    }
    return;
  }
  if (stat.isSymbolicLink()) {
    const target = fs.readlinkSync(source);
    const sourceParent = path.dirname(source);
    const absoluteTarget = path.isAbsolute(target)
      ? target
      : path.resolve(sourceParent, target);
    const checkedTarget = symlinkTargetForBoundaryCheck(sourceParent, target, absoluteTarget);
    if (!isPathInside(checkedTarget, sourceRoot)) {
      throw new Error(`Refusing to copy symlink outside source root: ${source}`);
    }
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.symlinkSync(target, destination);
    return;
  }
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(source, destination);
}

function updateCurrentSymlink(remoteRoot, versionPath) {
  fs.mkdirSync(remoteRoot, { recursive: true });
  const currentPath = path.join(remoteRoot, 'current');
  if (fs.existsSync(currentPath) || fs.lstatSync(currentPath, { throwIfNoEntry: false })) {
    const stat = fs.lstatSync(currentPath);
    if (!stat.isSymbolicLink()) {
      throw new Error(`Refusing to replace existing non-symlink current: ${currentPath}`);
    }
    fs.rmSync(currentPath, { force: true, recursive: false });
  }
  fs.symlinkSync(versionPath, currentPath, 'dir');
}

function logOperation(databasePath, type, skillName, status, message = '') {
  const db = initDatabase(databasePath);
  db.prepare(`
    INSERT INTO operations (type, skill_name, status, message, created_at)
    VALUES (?, ?, ?, ?, ?)
  `).run(type, skillName || null, status, message, new Date().toISOString());
  db.close();
}

function validateSkillName(name) {
  if (!name || name.includes('/') || name.includes('\\') || name === '.' || name === '..') {
    throw new Error(`Invalid skill name: ${name}`);
  }
  return name;
}

function unquoteYamlScalar(value) {
  if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
    return value.slice(1, -1);
  }
  return value;
}

function stripSkillMd(input) {
  return input.replace(/\/?SKILL\.md$/i, '');
}

function rejectRawUrlPathTraversal(input) {
  const rawPath = String(input)
    .replace(/^[^:]+:\/\/[^/]+\/?/, '')
    .split(/[?#]/)[0];
  for (const segment of rawPath.split('/')) {
    const normalized = segment.toLowerCase().replaceAll('%2e', '.');
    if (normalized === '.' || normalized === '..') {
      throw new Error('GitHub path must stay inside the repository');
    }
  }
}

function splitRefAndSkillPath(parts) {
  if (!parts.length) return { ref: 'main', path: '' };
  const skillRootIndex = knownSkillPathStart(parts);
  if (skillRootIndex !== -1) {
    return {
      ref: parts.slice(0, skillRootIndex).join('/'),
      path: parts.slice(skillRootIndex).join('/')
    };
  }
  return {
    ref: parts[0],
    path: parts.slice(1).join('/')
  };
}

function knownSkillPathStart(parts) {
  for (let index = 1; index < parts.length; index += 1) {
    if (parts[index] === 'skills') return index;
    const pair = `${parts[index]}/${parts[index + 1] || ''}`;
    if (['.agents/skills', '.codex/skills', '.claude/skills'].includes(pair)) return index;
  }
  return -1;
}

function validateRepoRelativePath(input) {
  if (!input || path.isAbsolute(input) || input.includes('\\')) {
    throw new Error('GitHub path must stay inside the repository');
  }
  for (const segment of input.split('/')) {
    if (!segment || segment === '.' || segment === '..') {
      throw new Error('GitHub path must stay inside the repository');
    }
  }
}

function validateGitReference(input) {
  if (!input || input.trim() === '') {
    throw new Error('Git reference is required');
  }
  if (input.trimStart().startsWith('-')) {
    throw new Error("Git reference must not start with '-'");
  }
}

function validateGitHubRepoUrl(input) {
  if (!input || input.trim() === '') {
    throw new Error('GitHub source is missing repoUrl');
  }
  if (input.trimStart().startsWith('-')) {
    throw new Error("Git remote URL must not start with '-'");
  }
  let url;
  try {
    url = new URL(input);
  } catch {
    throw new Error('Only https://github.com remote URLs are supported');
  }
  if (url.protocol !== 'https:' || url.hostname !== 'github.com') {
    throw new Error('Only https://github.com remote URLs are supported');
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error('GitHub remote URL must not contain credentials, query, or fragment');
  }
  const parts = url.pathname.split('/').filter(Boolean);
  if (parts.length !== 2 || parts.some((part) => part === '.' || part === '..' || part.startsWith('-'))) {
    throw new Error('GitHub remote URL must include a valid owner and repo');
  }
}

function validateRemoteSource(source) {
  if (source.type !== 'github') return;
  if (source.repoUrl) validateGitHubRepoUrl(source.repoUrl);
  if (source.path) validateRepoRelativePath(source.path);
  if (source.ref) validateGitReference(source.ref);
}

function isPathInside(child, root) {
  const relative = path.relative(root, child);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function symlinkTargetForBoundaryCheck(sourceParent, target, absoluteTarget) {
  try {
    return fs.realpathSync(absoluteTarget);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      if (!path.isAbsolute(target)) {
        return path.resolve(fs.realpathSync(sourceParent), target);
      }
      return path.resolve(absoluteTarget);
    }
    throw error;
  }
}

function trimGitSuffix(input) {
  return input.replace(/\.git$/i, '');
}

function sha256(content) {
  return crypto.createHash('sha256').update(content).digest('hex');
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}
