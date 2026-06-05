# Changelog

All notable changes to SkillBox will be documented in this file.

The format is based on Keep a Changelog, and this project uses semantic
version tags such as `v0.1.1`.

## Unreleased

- No unreleased changes.

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
