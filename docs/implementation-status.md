# Implementation Status

## Completed

- Created the monorepo layout for CLI/core, desktop app, and Rust crates.
- Added a Node-based core MVP using only built-in Node modules.
- Added SQLite-backed indexing with Node's built-in `node:sqlite`.
- Installed Rust stable with rustup.
- Migrated the core scan, import, symlink deploy, SQLite indexing, and GitHub URL parsing paths into Rust crates.
- Replaced the temporary system `sqlite3` shell-out with `rusqlite` parameterized writes.
- Installed `rustfmt` and verified Rust formatting.
- Switched the Tauri bridge from spawning the Node CLI to calling Rust crates directly.
- Implemented `SKILL.md` frontmatter parsing.
- Implemented recursive skill scanning.
- Implemented user and remote import storage.
- Implemented symlink deployment with overwrite protection.
- Implemented GitHub URL normalization for tree, blob, raw, and contents API URLs.
- Implemented a first CLI surface for the planned commands.
- Added a Tauri + React desktop shell with scan and path bridge commands.
- Implemented Rust/Tauri user-skills Git sync for the shared `~/SkillBox/user-skills` repository.
- Implemented Rust/Tauri/CLI remote skill update status checks and Dashboard status refresh.
- Added Rust crate scaffolding for the planned Tauri/Rust architecture.
- Verified the desktop shell in browser preview at `http://127.0.0.1:1420/`.

## Next Implementation Targets

- Add SQLite migrations and FTS search in the Rust core.
- Add network-backed remote update install and rollback flows to the desktop UI.
- Add import review screens for unknown existing skills.
- Add copy snapshot deployment mode after symlink mode is stable.
