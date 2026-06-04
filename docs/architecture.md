# SkillBox 架构地图

## 整体结构

SkillBox 当前是一个过渡期 monorepo。产品目标是管理跨 agent 的 skills、规则、提示词和能力包，
覆盖 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等主流 agent。

- `apps/desktop` 是 Tauri + React 桌面应用。
- `apps/desktop/src-tauri` 是 Tauri command 层，负责把 UI 请求转发到 Rust crates。
- `crates/skillbox-core` 是核心业务 crate，当前实现扫描、导入、候选导入、部署、SQLite 基础索引和偏好设置。
- `crates/skillbox-github` 负责 GitHub skill URL 解析和标准化。
- `crates/skillbox-git` 通过 `GitService` 负责 Rust 产品运行时的结构化 Git 调用和状态读取。
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
  -> crates/skillbox-core / skillbox-github / skillbox-git::GitService
  -> 本地文件系统、SQLite、GitHub URL 解析、结构化 Git 命令
```

当前 Tauri commands：

- `managed_paths` -> `skillbox_core::managed_paths`
- `managed_state` -> `skillbox_core::managed_state`
- `managed_preferences` -> `skillbox_core::managed_preferences`
- `set_skip_local_import_confirmation` -> `skillbox_core::set_skip_local_import_confirmation`
- `scan_skills` -> `skillbox_core::scan_skill_roots`
- `scan_import_candidates` -> `skillbox_core::scan_import_candidates`
- `scan_workspace_import_candidates` -> `skillbox_core::scan_import_candidates` scoped to one workspace root
- `import_candidates` -> `skillbox_core::import_candidates`
- `parse_github_url` -> `skillbox_github::parse_github_skill_url`
- `list_workspaces` -> `skillbox_core::list_workspaces`
- `scan_workspaces` -> `skillbox_core::scan_workspaces`
- `add_workspace` -> `skillbox_core::add_workspace`
- `forget_workspace` -> `skillbox_core::forget_workspace`
- `find_remote_source_candidates` -> `skillbox_core::find_remote_source_candidates`
- `preview_remote_source_binding` -> `skillbox_core::preview_remote_source_binding`
- `bind_remote_source` -> `skillbox_core::bind_remote_source`
- `list_remote_skill_versions` -> `skillbox_core::list_remote_skill_versions`
- `preview_remote_version_change` -> `skillbox_core::preview_remote_version_change`
- `apply_remote_version_change` -> `skillbox_core::apply_remote_version_change`
- `list_operations` -> `skillbox_core::list_operations`
- `list_history` -> `skillbox_core::list_history`
- `record_skill_usage` -> `skillbox_core::record_skill_usage`
- `usage_hook_statuses` -> `skillbox_core::usage_hook_statuses`
- `install_usage_hook` -> `skillbox_core::install_usage_hook`

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
- workspace registry 的发现、手动添加、扫描统计和 forget 操作。
- user/remote skill 导入。
- import candidates 扫描、类型推断、冲突检测。
- symlink 部署和部署索引。
- import backup 与 source 替换为 symlink。
- GitHub-only remote source search, manual binding, update check, version listing, diff preview, update/rollback apply, and operation logging.
- SQLite 基础表初始化和索引写入。
- 用户偏好读取与写入。
- skill usage 事件记录、聚合统计和 agent hook 注入配置。
- 未来应承载 agent adapter registry 和跨 agent 的规范化扫描/部署编排。

`skillbox-github` 负责：

- 接受 GitHub tree、blob、raw、contents API URL。
- 标准化 owner、repo、ref、path、repo URL 和展示 URL。
- 不负责下载、clone、稀疏 checkout 或版本历史写入。

`skillbox-git` 负责：

- 通过 `GitService` 作为 Rust 产品运行时唯一的 Git 服务边界。
- 用结构化参数执行 `git -C <repo> ...`，不拼接 shell 字符串。
- 读取仓库是否初始化、当前分支、dirty 状态和原始 status。
- 提供 init、origin 读取/设置、add、commit、push、`ls-remote` 等可复用 Git 原语。
- 集中处理 Git 网络命令的非交互环境变量、有界 timeout 和 stderr 返回。
- 不负责 managed store 级别的提交策略；`~/.skillbox/user-skills` 的同步编排在 `skillbox-core`。

legacy Node core 当前仍负责：

- `installRemoteSkillFromGitHub`
- Node 版 `operations` 日志写入

repo-local 开发脚本可以保留少量自用 Git 调用，例如 Git hooks 安装；这些不是 SkillBox 产品运行时边界。

GitHub install 迁移到 Rust 前，文档和测试需要明确标注当前入口，避免 UI 或 Rust CLI 声称已经完整覆盖。

## 真相源和部署目标

`~/.skillbox` 是 SkillBox 管理状态的真相源：

```text
~/.skillbox/
  user-skills/
  remote-skills/
  backups/
  skillbox.sqlite
```

Runtime 目录只是部署目标：

- `~/.codex/skills`
- `~/.agents/skills`
- `~/.claude/skills`
- 项目局部 `.codex/skills`
- 项目局部 `.agents/skills`
- 项目局部 `.claude/skills`
- Claude、OpenClaw、Cursor、Claude Code、Copilot 等 agent adapter 声明的全局或项目局部 target

Workspace registry 记录这些 skills root，作为后续 deploy skills 的目标候选。`global` workspace 表示
home-level agent root，`user` workspace 表示项目局部 root；React 只展示和提交结构化请求，发现、分类、持久化和按 workspace 扫描 import candidates 都在 Rust core。

不要在没有 adapter 语义的情况下猜测某个 agent 的目录布局。新增 agent 支持时，先定义 adapter 的发现路径、原生格式、部署方式和冲突处理。

默认部署方式是从 runtime 目录 symlink 到 managed store。runtime 目录中已有的非 symlink skill 不能被静默覆盖，导入或迁移时必须先备份或拒绝。

## 当前状态与目标状态

当前状态：

- Rust core 已经是桌面应用的主要后端。
- Rust CLI 有 `paths`、`scan`、`parse-github-url`、`import`、`deploy`、`user-skills-status`、`sync-user-skills`、`check-remote-updates`。
- Rust CLI 有 `remote-source-candidates`、`remote-source-preview`、`bind-remote-source`、`remote-versions`、`remote-preview-change`、`remote-apply-change`、`usage-record`、`usage-hook`、`usage-hook-status`、`usage-hook-install` 和 `operations`。
- Rust CLI 有 `workspaces`、`workspace-scan`、`workspace-add`、`workspace-forget` 来管理 workspace registry。
- Rust core 和 Tauri 已覆盖 `~/.skillbox/user-skills` 的共享 remote Git 同步。
- Rust core 和 Tauri 已覆盖 remote skill 的 GitHub update check、source binding、diff preview、update/rollback apply 和 operation log；Node CLI 仍保留 legacy GitHub install 入口。
- Rust core 和 Tauri 已覆盖 usage stats 显式上报，以及 Codex App、Codex CLI、Claude Code CLI 的 Stop hook 注入入口。
- agent support 当前主要是 `SKILL.md` / Codex-style roots，尚未覆盖 Claude、OpenClaw、Cursor、Claude Code、Copilot 的原生格式。
- Rust SQLite schema 和 Node SQLite schema 尚未完全统一。

目标状态：

- UI 和 CLI 都只通过 Rust core 执行业务逻辑。
- GitHub install 全部迁移到 Rust。
- 增加 agent adapter registry，让 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 runtime 通过同一 managed store 管理。
- Node CLI 退化为兼容包装或被移除。
- SQLite schema 由 Rust migration 管理，并兼容读取 Node MVP 已写入的数据。

本文件不记录逐步操作和字段细节；workflow 看 `docs/workflows.md`，存储字段看 `docs/data-model.md`。
