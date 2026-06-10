# Roadmap

SkillBox is early-stage software. This roadmap describes the public direction,
not a date-based commitment. Implementation details can change as the app gets
more real-world use.

## Current Focus: 0.1.x

The current release line focuses on making local skill management useful and
safe on macOS:

- keep `~/.skillbox` as the source of truth for managed skills;
- scan global and project-local `SKILL.md` workspaces;
- import local skills only after review;
- deploy managed skills through explicit symlink operations;
- track GitHub-backed remote skill sources, update status, diffs, updates, and
  rollbacks;
- sync user skills through a shared Git repository;
- record usage counts from supported local agent hooks without storing full
  transcripts;
- keep release, CI, and dependency hygiene visible.

## Near-Term Priorities

These are the next areas where focused contributions are most useful:

- **Rust CLI and desktop parity.** Continue moving legacy Node CLI behavior into
  Rust core and Tauri commands.
- **Search and navigation.** Add SQLite migrations and FTS-backed search for
  skills, operations, and usage history.
- **GitHub install flow.** Add network-backed GitHub install in Rust CLI and the
  desktop app with the same review-first safety model.
- **First-run onboarding.** Make scan, import, deploy, and backup implications
  clearer for new users.
- **Dependency hygiene.** Keep Tauri, Vite, Rust crates, and GitHub Actions
  current without weakening the local safety model.
- **Documentation polish.** Keep screenshots, install instructions, and safety
  expectations aligned with the latest release.

## Good First Contribution Areas

Good first issues should be small, testable, and low-risk. The best starter
work usually lives in:

- documentation fixes and screenshots;
- issue templates and contributor guidance;
- focused UI copy or empty-state polish;
- tests for existing helpers;
- small CLI or normalization improvements that do not touch destructive file
  operations.

See [Good first issues](good-first-issues.md) for contributor and maintainer
guidance.

## Later Directions

These are important, but need more design or production feedback before they
should become starter work:

- native adapters for Claude, OpenClaw, Cursor, Claude Code, Copilot, and other
  agent ecosystems;
- copy-snapshot deployment mode in addition to symlink deployment;
- broader CLI packaging and distribution beyond the current macOS desktop
  release;
- Windows and Linux support evaluation;
- richer backup, restore, and audit workflows for managed skill changes.

## Non-Goals

SkillBox should not become:

- a hosted cloud account or remote synchronization service;
- an automatic executor of arbitrary user-provided shell strings;
- a tool that silently overwrites existing runtime content;
- an agent-specific format that treats one runtime as the global source of
  truth.
