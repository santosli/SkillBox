# SkillBox 架构地图

## 整体结构

SkillBox 当前是一个过渡期 monorepo。产品目标是管理跨 agent 的 skills、规则、提示词和能力包，
覆盖 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等主流 agent。

- `apps/desktop` 是 Tauri + React 桌面应用。
- `apps/desktop/src-tauri` 是 Tauri command 层，负责把 UI 请求转发到 Rust crates。
- `crates/skillbox-core` 是核心业务 crate，当前实现扫描、导入、候选导入、部署、SQLite 基础索引和偏好设置。
- `crates/skillbox-github` 负责 GitHub skill URL 解析和标准化。
- `crates/skillbox-git` 负责结构化调用 Git 并读取状态。
- `crates/skillbox-cli` 是 Rust CLI，目标是和桌面应用共享同一套 Rust core。
- `packages/skillbox-core` 和 `packages/skillbox-cli` 是 legacy Node MVP，仍保留部分尚未迁移到 Rust 的能力。

新增业务能力优先进入 Rust crates。Node 代码可以作为行为参考和兼容入口，但不应成为新功能的主实现位置。

跨 agent 支持应通过 adapter 层表达：

- managed store 保存 SkillBox 的规范化状态，不绑定任何单一 agent。
- agent adapter 负责发现某类 runtime、读取该 agent 的原生格式、转换为 SkillBox 可管理的记录、并部署回该 agent 需要的路径或文件形态。
- 当前实现只覆盖 `SKILL.md` 目录和 `.codex/.agents` 风格 runtime；不要把这当成最终格式边界。

## 调用关系

桌面应用调用链：

```text
React UI
  -> @tauri-apps/api/core invoke(...)
  -> apps/desktop/src-tauri/src/lib.rs Tauri command
  -> crates/skillbox-core / skillbox-github
  -> 本地文件系统、SQLite、GitHub URL 解析
```

当前 Tauri commands：

- `managed_paths` -> `skillbox_core::managed_paths`
- `managed_state` -> `skillbox_core::managed_state`
- `managed_preferences` -> `skillbox_core::managed_preferences`
- `set_skip_local_import_confirmation` -> `skillbox_core::set_skip_local_import_confirmation`
- `scan_skills` -> `skillbox_core::scan_skill_roots`
- `scan_import_candidates` -> `skillbox_core::scan_import_candidates`
- `import_candidates` -> `skillbox_core::import_candidates`
- `parse_github_url` -> `skillbox_github::parse_github_skill_url`

Rust CLI 当前调用链：

```text
cargo run -p skillbox-cli --offline -- <command>
  -> crates/skillbox-cli
  -> skillbox-core / skillbox-github
```

Node CLI 当前调用链：

```text
node packages/skillbox-cli/bin/skillbox.js <command>
  -> packages/skillbox-core/index.js
  -> Node fs / node:sqlite / git execFileSync
```

## 模块边界

`skillbox-core` 负责：

- skill 根目录扫描和 `SKILL.md` 读取。
- managed store 路径计算和初始化。
- user/remote skill 导入。
- import candidates 扫描、类型推断、冲突检测。
- symlink 部署和部署索引。
- import backup 与 source 替换为 symlink。
- SQLite 基础表初始化和索引写入。
- 用户偏好读取与写入。
- 未来应承载 agent adapter registry 和跨 agent 的规范化扫描/部署编排。

`skillbox-github` 负责：

- 接受 GitHub tree、blob、raw、contents API URL。
- 标准化 owner、repo、ref、path、repo URL 和展示 URL。
- 不负责下载、clone、稀疏 checkout 或版本历史写入。

`skillbox-git` 负责：

- 用结构化参数执行 `git -C <repo> ...`。
- 读取仓库是否初始化、当前分支、dirty 状态和原始 status。
- 不负责提交策略、remote 配置或 push 工作流。

legacy Node core 当前仍负责：

- `installRemoteSkillFromGitHub`
- `checkRemoteUpdates`
- `rollbackRemoteSkill`
- `syncUserSkills`
- Node 版 `operations` 日志写入

这些能力迁移到 Rust 前，文档和测试需要明确标注当前入口，避免 UI 或 Rust CLI 声称已经完整覆盖。

## 真相源和部署目标

`~/SkillBox` 是 SkillBox 管理状态的真相源：

```text
~/SkillBox/
  user-skills/
  remote-skills/
  backups/
  skillbox.sqlite
```

Runtime 目录只是部署目标：

- `~/.codex/skills`
- `~/.agents/skills`
- 项目局部 `.codex/skills`
- 项目局部 `.agents/skills`
- Claude、OpenClaw、Cursor、Claude Code、Copilot 等 agent adapter 声明的全局或项目局部 target

不要在没有 adapter 语义的情况下猜测某个 agent 的目录布局。新增 agent 支持时，先定义 adapter 的发现路径、原生格式、部署方式和冲突处理。

默认部署方式是从 runtime 目录 symlink 到 managed store。runtime 目录中已有的非 symlink skill 不能被静默覆盖，导入或迁移时必须先备份或拒绝。

## 当前状态与目标状态

当前状态：

- Rust core 已经是桌面应用的主要后端。
- Rust CLI 有 `paths`、`scan`、`parse-github-url`、`import`、`deploy`。
- Node CLI 仍有更完整的远程 GitHub 和 user-skills Git 工作流。
- agent support 当前主要是 `SKILL.md` / Codex-style roots，尚未覆盖 Claude、OpenClaw、Cursor、Claude Code、Copilot 的原生格式。
- Rust SQLite schema 和 Node SQLite schema 尚未完全统一。

目标状态：

- UI 和 CLI 都只通过 Rust core 执行业务逻辑。
- GitHub install、update check、update、rollback、user-skills Git sync 全部迁移到 Rust。
- 增加 agent adapter registry，让 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 runtime 通过同一 managed store 管理。
- Node CLI 退化为兼容包装或被移除。
- SQLite schema 由 Rust migration 管理，并兼容读取 Node MVP 已写入的数据。

本文件不记录逐步操作和字段细节；workflow 看 `docs/workflows.md`，存储字段看 `docs/data-model.md`。
