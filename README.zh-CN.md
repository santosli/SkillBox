# SkillBox

> 管理本地 `SKILL.md` agent runtimes 中的 skills。

[English](README.md) | 简体中文

[官网](https://santosli.github.io/SkillBox/) | [最新版本](https://github.com/santosli/SkillBox/releases/latest) | [GitHub](https://github.com/santosli/SkillBox)

![状态](https://img.shields.io/badge/status-macOS%20release-blue)
![平台](https://img.shields.io/badge/platform-macOS-111827)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB)
![Rust](https://img.shields.io/badge/Rust-core-B7410E)
![Frontend](https://img.shields.io/badge/Frontend-React%20%2B%20Vite-61DAFB)

![SkillBox dashboard](docs/screenshots/skillbox-dashboard-v041.jpg)

SkillBox 是一个 local-first 的 macOS 桌面应用，带 Rust core/CLI，用来管理基于 `SKILL.md` 的 skill 与能力包，同时避免把任一受支持的 agent runtime 当作唯一真相源。

当前版本：`v0.4.3`。SkillBox 现在已经可以用于本地 skill 管理，但仍是早期软件。重要 skills 请保留备份，并在应用每一次文件系统变更前先 review。

## 宣传视频

[![观看 SkillBox 宣传视频](docs/promo/skillbox-intro/skillbox-promo-poster.jpg)](docs/promo/skillbox-intro/skillbox-promo.mp4)

30 秒快速了解 SkillBox：本地优先的 skill 管理、导入前审核、远程更新、使用历史和 GitHub 发布。

## 为什么

- **一个 managed store，面向受支持的 runtime。** 把持久 skill 状态放在 `~/.skillbox`，再部署到受支持的全局或项目局部 `SKILL.md` roots。
- **完整生命周期都先 review。** 导入、部署、类型迁移、source 绑定、更新、回滚和删除都会先展示影响，再改 managed store 或 runtime 文件。
- **远程 skill 版本管理。** SkillBox 打开期间检查 GitHub source，预览全文件 diff，应用更新，并回滚到不可变版本。
- **Git 提交和推送也可审查。** 查看 user-skill diff、创建 Conventional Commit，并可选推送；远端历史分叉仍在 SkillBox 外按正常 Git 冲突处理。
- **真实调用与操作历史。** 通过支持的 agent hooks 记录 skill 调用，与管理操作统一展示，同时不保存完整聊天 transcript。
- **安全的存储与部署默认值。** 使用顺序 SQLite migrations、恢复备份、完整性检查和 ownership-checked symlink，不静默覆盖 runtime 内容。
- **签名的 macOS 分发。** 可安装已公证 DMG 或 Homebrew cask，app 更新也只在用户确认后应用。

## 截图

![SkillBox skill detail](docs/screenshots/skillbox-skill-detail-v041.jpg)

Dashboard 支持本地搜索、类型/更新/tag/favorite 过滤以及 grid/list 切换，并用状态优先的卡片展示结果。详情页集中展示 workspace 部署、调用统计、版本历史、source 绑定、rollback、标签、类型迁移和经审查的删除操作。

![SkillBox workspaces](docs/screenshots/skillbox-workspaces-v041.jpg)

Workspaces 视图会跟踪全局和项目局部 `SKILL.md` roots，包括 Codex CLI、Codex App、Claude Code skill folders 和项目自己的 runtime。可以按 workspace 名称、路径或 agent 搜索，并与 Global/User 类型筛选组合使用。

![SkillBox history](docs/screenshots/skillbox-history-v041.jpg)

History 会把真实 skill 调用和管理操作合并展示。hook 提供 prompt 文本时，SkillBox 只保存最多 500 字符的截断片段，而不是完整 transcript；这个片段仍可能包含用户输入。

![SkillBox managed store health](docs/screenshots/skillbox-settings-health-v041.jpg)

Doctor 会检查 SQLite schema 与完整性、managed skills、deployments、workspaces 和 import backups。诊断是只读操作，清理 stale deployment records 需要用户显式执行 repair。

![SkillBox import review](docs/screenshots/skillbox-import-review.jpg)

Import review 让本地扫描结果保持显式可审查：候选项会先完成分类，然后 SkillBox 才会把它们复制进 managed store。多个 runtime root 中导入内容完全一致的副本会合并为一条 review 记录并保留所有来源位置，但本次只导入 primary，其他副本保持不变；脚本或资源不同的 skill 会继续分开显示。

## SkillBox 管什么

SkillBox 默认把 managed store 放在 `~/.skillbox`：

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

后续支持 Claude、OpenClaw、Cursor、Claude Code、Copilot 和其它非 `SKILL.md` 原生格式时，应通过明确的 agent adapter 表达，而不是把 agent-specific 行为硬编码在 UI 里。

## 功能

- 扫描并登记受支持的全局或项目局部 `SKILL.md` workspaces；按名称、路径或 agent 搜索，并按 scope 过滤。
- 在复制前 review user、remote 和 system import candidates；合并导入内容一致的多 root 副本但不丢失来源位置，并对符合条件的 deploy-back import 执行保守回退。
- 通过 preview/apply 安装 GitHub-backed skill，并在不替换当前版本的情况下绑定识别到的 remote source candidate。
- 检查 remote source、预览全文件 diff、应用更新，并回滚到不可变版本。
- 通过 ownership-checked symlink 在单个 workspace 部署或移除 managed skill；经 review 迁移 User/Remote 类型并重定向 deployments。
- 名称确认后，从 managed store 和全部关联 workspace 删除 skill，同时保留 recovery backup 和 workspace registrations。
- Review user-skill Git diff、为选中文件创建 Conventional Commit，并可选推送，不尝试自动合并远端变更。
- 按类型、更新状态、tag 或 favorite 搜索过滤 Dashboard，在 grid/list 间切换，并把 favorites 与 tags 持久化到 SQLite。
- 记录受支持的 Codex App、Codex CLI 和 Claude Code CLI hook 调用，与管理操作一同浏览，不保存完整 transcript。
- 执行顺序 SQLite migrations、迁移前备份和完整性检查；运行 Doctor 诊断并显式清理 stale deployment records。
- 检查已签名的 GitHub Releases，并只在用户确认后安装 macOS app 更新。

## 依赖

- macOS 14 Sonoma 或更新版本
- Git，用于 user-skill sync 和 remote skill workflows
- 使用 `SKILL.md` 目录的 agent runtime

Windows、Linux 和 Homebrew CLI formula 不属于当前版本范围。

## 官网遥测

SkillBox 官网仅在访客主动同意后使用可选的 VibeLoft 页面访问遥测。该网站接入与 macOS 应用和 CLI 相互独立，无法访问 managed skills、prompt、runtime 目录或本地 SkillBox 数据库。上报字段和退出方式见[官网隐私说明](https://santosli.github.io/SkillBox/privacy.html)。

## 安装

### GitHub Releases

从 GitHub Releases 下载已签名并公证的 DMG：

https://github.com/santosli/SkillBox/releases

本次发布使用这个 asset：

```text
SkillBox_0.4.3_universal.dmg
```

对应 checksum：

```text
SkillBox_0.4.3_universal.dmg.sha256
```

打开 DMG，把 `SkillBox.app` 拖到 `/Applications`。

通过 DMG 安装的 app 可以在 Settings -> App updates 检查已签名的
GitHub Releases，并在确认后安装更新。

### Homebrew

Homebrew cask 使用项目自己的 tap，而不是官方 Homebrew Cask 仓库：

```sh
brew tap santosli/tap
brew install --cask skillbox
```

升级：

```sh
brew upgrade --cask skillbox
```

卸载：

```sh
brew uninstall --cask skillbox
```

Homebrew uninstall 不会删除 `~/.skillbox`。

## 首次使用

1. 打开 SkillBox。
2. 点击 `Scan` 发现已知的全局和项目局部 skill workspaces。
3. 使用 `Import` 先 review 候选项，再让 SkillBox 复制到 `~/.skillbox`。
4. 使用 `Install` 先预览 GitHub-backed remote skills，再确认是否复制到 managed store。
5. 把 managed skills 部署到选定 runtime workspaces。
6. 可选：在 Settings 启用 usage hook injection，用来记录真实 skill 调用。

## 权限和本地变更

SkillBox 是 local-first，不需要托管账号。应用可能会：

- 扫描已知 runtime 目录中的 `SKILL.md` folders；
- 在 `~/.skillbox` 下写入 managed copies 和 metadata；
- 创建从 runtime 目录回指到 managed skills 的 symlink；
- 为 `~/.skillbox/user-skills` 初始化和更新 Git metadata；
- 在你明确注入 hooks 时，修改受支持 runtime 的 hook config files。

SkillBox 会把 runtime folders、GitHub URLs、下载归档和既有 skills 都视为不可信输入，不应静默覆盖非 symlink runtime target。

## 卸载和重置

见 [docs/uninstall-reset.md](docs/uninstall-reset.md)，其中包含删除应用、回滚 hook injection、删除 runtime symlinks，以及可选删除 managed store 的步骤。

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
crates/skillbox-core/      managed skill lifecycle, safety, SQLite, workspaces, history, hooks, and Git sync
crates/skillbox-github/    GitHub skill URL parsing and normalization
crates/skillbox-git/       structured Git service boundary
crates/skillbox-cli/       Rust CLI
docs/                      architecture, data model, workflows, ADRs
```

新增核心业务逻辑应进入 Rust crates。React 应调用结构化 Tauri commands，不应直接拥有文件系统、Git、GitHub 下载、迁移或回滚行为。

## 文档

- [Roadmap](docs/roadmap.md)
- [Good first issues](docs/good-first-issues.md)
- [Architecture](docs/architecture.md)
- [Data model](docs/data-model.md)
- [Workflows](docs/workflows.md)
- [Implementation status](docs/implementation-status.md)
- [Contributing](CONTRIBUTING.md)
- [Managed store ADR](docs/decisions/0001-managed-store-is-source-of-truth.md)
- [Symlink deployment ADR](docs/decisions/0002-symlink-deployment-by-default.md)
- [Rust core migration ADR](docs/decisions/0003-migrate-node-cli-behavior-to-rust-core.md)
- [Agent adapter ADR](docs/decisions/0004-support-multiple-agent-runtimes-through-adapters.md)

## 开发

本地 setup、测试命令、release invariants 和贡献规范见 [CONTRIBUTING.md](CONTRIBUTING.md)。
新贡献者可以先看 [Good first issues](docs/good-first-issues.md) 或公开 [Roadmap](docs/roadmap.md)。

常用命令：

```sh
npm test
cargo test --offline
npm --workspace apps/desktop run build
npm run docs:check-staged
```

涉及 UI 变更时，也需要运行 Vite 或 Tauri app 并手动验证受影响页面。

## License

SkillBox 使用 [MIT License](LICENSE)。
