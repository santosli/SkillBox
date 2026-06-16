use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, String>;

const DEFAULT_STATUS_REFRESH_INTERVAL_MINUTES: u32 = 5;
const MIN_STATUS_REFRESH_INTERVAL_MINUTES: u32 = 1;
const MAX_STATUS_REFRESH_INTERVAL_MINUTES: u32 = 1440;
const DEFAULT_REMOTE_UPDATE_TIMEOUT_SECONDS: u32 = 30;
const MIN_REMOTE_UPDATE_TIMEOUT_SECONDS: u32 = 5;
const MAX_REMOTE_UPDATE_TIMEOUT_SECONDS: u32 = 300;
const REMOTE_UPDATE_CHECK_CONCURRENCY: usize = 3;
const CLAUDE_MARKETPLACE_SKILLS_API: &str = "https://claudemarketplaces.com/api/skills";
const MAX_TEXT_DIFF_PREVIEW_BYTES: usize = 1024 * 1024;
const MAX_USAGE_METADATA_JSON_BYTES: usize = 16 * 1024;
const MAX_USAGE_PROMPT_EXCERPT_CHARS: usize = 500;
const DEFAULT_USER_SKILLS_GITIGNORE: &str = "\
# macOS
.DS_Store
.AppleDouble
.LSOverride
._*

# Python
__pycache__/
*.py[cod]
*$py.class
.pytest_cache/
.mypy_cache/
.ruff_cache/
.venv/
venv/

# JavaScript
node_modules/
npm-debug.log*
yarn-debug.log*
yarn-error.log*
pnpm-debug.log*

# Local config and logs
.env
.env.*
!.env.example
*.log

# Temporary files
*.tmp
*.temp
.tmp/
tmp/
";
const USAGE_METADATA_CONTENT_KEYS: &[&str] = &[
    "prompt",
    "content",
    "messages",
    "transcript",
    "input",
    "output",
    "diff",
    "file_contents",
];

mod db;
mod fsutil;
mod git_sync;
mod hooks;
mod import;
mod marketplace;
mod operations;
mod paths;
mod remote;
mod skills;
mod state;
mod types;
mod usage;
mod workspaces;

pub(crate) use db::*;
pub(crate) use fsutil::*;
pub use git_sync::*;
pub use hooks::*;
pub use import::*;
pub(crate) use marketplace::*;
pub use operations::*;
pub use paths::*;
pub use remote::*;
pub use skills::*;
pub use state::*;
pub use types::*;
pub use usage::*;
pub use workspaces::*;

#[cfg(test)]
mod tests;
