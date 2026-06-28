# Changelog

All notable changes to SkillBox will be documented in this file.

The format is based on Keep a Changelog, and this project uses semantic
version tags such as `v0.3.0`.

## Unreleased

- No unreleased changes.

## 0.3.8

- Add Import Revert for deploy-back imports so a runtime skill can be restored to its pre-import folder.
- Preserve remote managed versions during revert and allow the same skill to be imported and reverted again.
- Block unsafe import reverts when a skill has multiple workspace deployments or the source/backup no longer matches the recorded state.
- Add CLI, desktop bridge, and Skill Detail controls for reviewing and confirming import reverts.

## 0.3.7

- Add workspace skill review tabs that separate unimported, imported, and system candidates while preserving symlink-only workspace skills.
- Reuse the searchable skill review list in Import Review, hiding duplicate symlink candidates when their source skill is already present.
- Allow managed skills to be changed between User and Remote storage with a confirmation flow that retargets existing workspace deployments.

## 0.3.6

- Fix a desktop startup blank screen caused by duplicate React runtimes after dependency updates.
- Keep the desktop React dependency resolved to a single runtime so icon rendering does not crash the app.

## 0.3.5

- Move active workspace agent icons next to the Active workspaces label in the skill detail deployment panel.
- Keep the active workspace icon stack vertically centered with the label text.

## 0.3.4

- Rename the remote skill update confirmation button to Apply Update.
- Refresh only the updated remote skill status after applying a version change to avoid unnecessary dashboard stalls.
- Preserve the rest of the remote update status table during targeted refreshes.

## 0.3.3

- Share the Dashboard page title template across Dashboard, Settings, Workspaces, and History for consistent page headers.
- Compact the Settings page into a clearer tabbed workbench with stacked setting groups and no duplicate status summary.
- Improve remote diff review readability by explaining omitted oversized previews and separating footer actions from the diff pane.
- Add the SkillBox promo video and source package to the public documentation.

## 0.3.2

- Install GitHub-backed remote skills from the desktop Install dialog without deploying them automatically.
- Stop counting managed remote skill `current` symlinks as active runtime workspaces.
- Keep newly imported skill tags empty until users add their own labels.
- Align dashboard page title spacing with the sidebar brand.

## 0.3.1

- Fix remote update previews when a version diff includes directory entries.
- Fix applying remote updates for skills that symlink to shared directories inside the same GitHub repository.
- Preserve symlink escape protections for local imports and external paths while snapshotting safe same-repo shared files.

## 0.3.0

- Add signed in-app update checks for the macOS desktop app, with
  user-confirmed install and restart.
- Publish Tauri updater artifacts and `latest.json` alongside the signed DMG in
  the release workflow.
- Build both macOS app and DMG bundles in the release workflow so updater
  archives are generated, verified, and published.
- Upload updater artifacts with versioned asset filenames so `latest.json`
  update URLs match the GitHub Release downloads.
- Extend release automation and documentation so app updater assets are verified
  before Homebrew publication.
- Upgrade the desktop build tooling to Vite 8 to clear high-severity npm audit
  findings before release.

## 0.2.0

- Retire the legacy Node CLI/core packages and move GitHub install, rollback,
  update checking, and compatibility command entry points onto the Rust
  CLI/core path.
- Strengthen CI and dependency governance with Rust clippy warnings-as-errors,
  Rust and npm audit jobs, Dependabot configuration, and a PR template.
- Add public project roadmap and good-first-issue guidance for contributors.
- Align public release, security, contribution, architecture, and workflow docs
  with the Rust-only CLI/core direction.
- Improve desktop maintainability by splitting large UI and core modules, and
  link the sidebar Help action to GitHub Issues.

## 0.1.1

- Promote the macOS app from public alpha to the first regular release.
- Improve workspace scans, SKILL.md description parsing, user skill sync
  defaults, remote update detection, dashboard tagging, and desktop layout.
- Update release automation to publish regular releases while keeping alpha tag
  support.

## 0.1.0-alpha.3

- Prepare public alpha documentation, CI, release workflow, and Homebrew cask
  template.
- Add mounted DMG signature, Gatekeeper, version, and bundle identifier checks
  before publishing release assets.

## 0.1.0-alpha.1

Planned first public alpha.

- Local macOS desktop app for `SKILL.md`-based skill management.
- Scan global and project-local runtime skill directories.
- Import local and remote skills into `~/.skillbox`.
- Deploy managed skills to runtime directories with symlinks.
- Track remote skill sources and versions.
- Optional usage hook injection for local call counting.
