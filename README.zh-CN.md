# SkillBox

> 管理本地多个 agent runtime 中的 skills。

[English](README.md) | 简体中文

![状态](https://img.shields.io/badge/status-local--first%20MVP-blue)
![平台](https://img.shields.io/badge/platform-macOS-111827)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-core-B7410E)
![Node.js](https://img.shields.io/badge/Node.js-legacy%20CLI-43853D)

![SkillBox dashboard](docs/screenshots/skillbox-dashboard.png)

SkillBox 是一个本地工具，用来管理基于 `SKILL.md` 的 skills、规则、提示词和能力包，同时避免把某一个 agent runtime 当作唯一真相源。用户自己创建的 skills 存放在 `~/.skillbox/user-skills`，远程 skills 以版本快照形式存放在 `~/.skillbox/remote-skills`，`~/.codex/skills`、`~/.agents/skills`、`~/.claude/skills` 等 agent runtime 目录只是部署目标。

当前项目包含 Tauri + React 桌面壳、承载核心 workflow 的 Rust crates、Rust CLI，以及仍保留 GitHub install 过渡入口的 legacy Node CLI。

## 为什么

- **一个 managed store，面向多个 runtime。** 把持久 skill 状态放在 `~/.skillbox`，再按需部署到各个 agent runtime。
- **本地 skills 一键同步。** 在桌面应用里直接提交并推送 user skill 变更，不需要离开 SkillBox。
- **远程 skills 定时检查。** 自动刷新 remote skill 状态，有可用更新时先 review，再应用。
- **统计真实 skill 调用。** 通过支持的 agent hooks 记录 skill 调用，并在卡片和 History 里展示调用次数。
- **远程 skill 版本管理。** 预览 diff、应用更新，并能回滚到不可变的 remote skill 版本。
- **导入前先审查。** 本地扫描候选会先被分类为 user、remote 或 system，然后 SkillBox 才会复制内容。
- **安全的默认部署。** 默认用 symlink 部署，并拒绝静默覆盖 runtime 中已有内容。

## 截图

![SkillBox skill card detail](docs/screenshots/skillbox-dashboard-card.png)

Skill card 会把调用次数、更新状态、标签、收藏状态和已部署 runtime target 放在同一个卡片里，方便快速判断维护状态。

![SkillBox workspaces](docs/screenshots/skillbox-workspaces.png)

Workspaces 视图会跟踪全局和项目局部 skill roots，包括 Codex CLI、Claude Code、Codex App 和项目自己的 runtime。

![SkillBox history](docs/screenshots/skillbox-history.png)

History 会把真实 skill 调用和管理操作合并展示，prompt 只保留经过压缩和脱敏的小片段。

![SkillBox import review](docs/screenshots/skillbox-import-review.jpg)

Import review 让本地扫描结果保持显式可审查：候选项会先完成分类，然后 SkillBox 才会把它们复制进 managed store。

## SkillBox 管什么

```text
~/.skillbox/
  user-skills/
    <skill-name>/
      SKILL.md
  remote-skills/
    <skill-name>/
      source.json
      current -> versions/<version>
      versions/
        <version>/
          SKILL.md
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

后续支持 Claude、OpenClaw、Cursor、Claude Code、Copilot 和其它原生格式时，应通过明确的 agent adapter 表达，而不是把 agent-specific 行为硬编码在 UI 里。

## 功能

- 扫描本地 `SKILL.md` roots，返回按名称排序的 skills、frontmatter 元数据、content hash、symlink 状态和扫描错误。
- 把既有本地 skills 导入到 `~/.skillbox/user-skills` 或 `~/.skillbox/remote-skills`。
- 通过 symlink 把 managed skills 部署到 runtime 目录，也支持 undeploy。
- 解析指向 skill 目录或 `SKILL.md` 的 GitHub tree、blob、raw 和 contents API URL。
- 跟踪远程 GitHub source，检查更新，预览全文件 diff，应用更新，并回滚到不可变版本。
- 管理全局和项目局部 runtime 的 workspace roots。
- 通过共享 Git 仓库同步 user skills，并在桌面端提供 diff review 和 Conventional Commit message 生成。
- 通过 Codex App、Codex CLI、Claude Code CLI hooks 记录 usage events，但不保存完整聊天正文。
- 从 SQLite 记录中浏览桌面 operation 和 usage history。

## 安装

SkillBox 目前从源码运行。这个仓库还没有打包 release。

### 依赖

- macOS
- Node.js 和 npm
- Rust stable 和 cargo
- Git
- 通过 `apps/desktop` workspace dependency 使用的 Tauri CLI

如果 fresh shell 里找不到 `cargo`，先加载 Rust 环境：

```sh
source ~/.cargo/env
```

### 本地 checkout

```sh
npm install
npm run hooks:install
cargo test --offline
npm test
```

`npm install` 会运行 hook installer，把 Git 指向仓库里的 `.githooks/` 目录。pre-commit hook 会检查 staged implementation 或 workflow 变更是否需要同步更新文档。

## 桌面应用

运行浏览器预览：

```sh
npm --workspace apps/desktop run dev
```

运行 Tauri 桌面应用：

```sh
npm --workspace apps/desktop run tauri dev
```

`tauri dev` 会加载 `http://127.0.0.1:1420`。Vite dev server 使用 `--strictPort`，因此启动桌面应用前需要确保 `1420` 端口空闲。

构建前端：

```sh
npm --workspace apps/desktop run build
```

## CLI

Rust CLI 是目标 CLI surface：

```sh
cargo run -p skillbox-cli -- paths
cargo run -p skillbox-cli -- scan ~/.codex/skills ~/.agents/skills ~/.claude/skills
cargo run -p skillbox-cli -- import ./path/to/skill --type user
cargo run -p skillbox-cli -- deploy my-skill --target ~/.codex/skills
cargo run -p skillbox-cli -- workspace-scan
cargo run -p skillbox-cli -- check-remote-updates
```

Legacy Node CLI 仍作为兼容入口保留，并承载当前 GitHub install workflow：

```sh
node packages/skillbox-cli/bin/skillbox.js scan --json
node packages/skillbox-cli/bin/skillbox.js paths --json
node packages/skillbox-cli/bin/skillbox.js parse-github-url <github-skill-url> --json
node packages/skillbox-cli/bin/skillbox.js install <github-skill-url> --json
```

## 架构

```text
React desktop UI
  -> Tauri commands
  -> skillbox-core / skillbox-github / skillbox-git
  -> local filesystem, SQLite, Git, and structured GitHub source metadata
```

Workspace 布局：

```text
apps/desktop/              Tauri + React desktop app
apps/desktop/src-tauri/    Tauri command bridge
crates/skillbox-core/      scan, import, deploy, SQLite, workspaces, updates, hooks
crates/skillbox-github/    GitHub skill URL parsing and normalization
crates/skillbox-git/       structured Git service boundary
crates/skillbox-cli/       Rust CLI
packages/skillbox-core/    legacy Node core
packages/skillbox-cli/     legacy Node CLI
docs/                      architecture, data model, workflows, ADRs
```

新增核心业务逻辑应进入 Rust crates。React 应调用结构化 Tauri commands，不应直接拥有文件系统、Git、GitHub 下载、迁移或回滚行为。

## 安全模型

- `~/.skillbox` 是 managed source of truth。
- Runtime folders 是部署目标，不是持久状态。
- 既有非 symlink runtime skills 不会被静默覆盖。
- Import replacement path 必须保留备份或拒绝操作。
- GitHub URL、下载内容、外部路径和 runtime skills 都是不可信输入。
- 产品代码使用结构化参数调用 Git 和外部命令，不执行用户提供的 shell 字符串。
- Usage hooks 只记录小型 usage events 和 prompt excerpts，不记录完整聊天正文。

## 文档

- [Architecture](docs/architecture.md)
- [Data model](docs/data-model.md)
- [Workflows](docs/workflows.md)
- [Implementation status](docs/implementation-status.md)
- [Contributing](CONTRIBUTING.md)
- [Managed store ADR](docs/decisions/0001-managed-store-is-source-of-truth.md)
- [Symlink deployment ADR](docs/decisions/0002-symlink-deployment-by-default.md)
- [Rust core migration ADR](docs/decisions/0003-migrate-node-cli-behavior-to-rust-core.md)
- [Agent adapter ADR](docs/decisions/0004-support-multiple-agent-runtimes-through-adapters.md)

## 开发检查

```sh
npm test
cargo test --offline
npm --workspace apps/desktop run build
npm run docs:check-staged
```

如果改动 UI，还需要运行 Vite 或 Tauri 应用，并手动验证受影响页面。

## 当前边界

- 第一阶段实现聚焦 `SKILL.md` 目录和 Codex-style runtime roots。
- GitHub install 仍是 legacy Node CLI workflow；Rust 已覆盖 URL 解析、remote source binding、update check、version preview、update apply、rollback apply 和 operation logging。
- Rust 和 Node SQLite schema 尚未完全统一。
- Copy-snapshot deployment 和非目录型 skill 原生 adapter 属于后续工作。

## License

当前仓库还没有 license 文件。
