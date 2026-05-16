# SkillBox

SkillBox is a local macOS app and CLI for managing Codex-compatible skills.

The project is currently bootstrapped with:

- A dependency-free Node CLI MVP that can scan, index, import, deploy, and parse GitHub skill URLs.
- A Tauri + React desktop shell wired directly to Rust commands.
- Rust crates for core scanning/import/deploy behavior, GitHub URL parsing, Git status, and CLI access.

## Current Commands

```sh
npm test
node packages/skillbox-cli/bin/skillbox.js scan --json
node packages/skillbox-cli/bin/skillbox.js paths --json
node packages/skillbox-cli/bin/skillbox.js parse-github-url <github-url> --json
cargo run -p skillbox-cli --offline -- scan ~/.codex/skills ~/.agents/skills
cargo test --offline
```

The default managed root is `~/SkillBox`, or `SKILLBOX_HOME` when set.

## Managed Layout

```text
~/SkillBox/
  user-skills/
  remote-skills/
  skillbox.sqlite
```

Runtime directories such as `~/.codex/skills` and `~/.agents/skills` are deployment targets.
SkillBox deploys managed skills through symlinks by default.

## Toolchain Status

Rust is installed through rustup on this machine. In fresh shells, use `source ~/.cargo/env`
or call `/Users/santos/.cargo/bin/cargo` directly if `cargo` is not yet on `PATH`.
