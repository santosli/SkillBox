# SkillBox 开发指南

## 环境

需要的工具：

- Node.js 和 npm，用于 legacy Node CLI、React/Vite 和 Tauri 前端依赖。
- Rust stable 和 cargo，用于 Rust crates、Rust CLI 和 Tauri 后端。
- Tauri CLI，通过 `apps/desktop` workspace 的 `@tauri-apps/cli` 运行。
- Git，用于 user-skills sync、GitHub install/update 相关工作流。

本机注意事项：

- Rust 通过 rustup 安装。
- fresh shell 中如果找不到 `cargo`，先运行 `source ~/.cargo/env`。
- 也可以直接调用 `/Users/santos/.cargo/bin/cargo`。

## 常用命令

测试：

```sh
npm test
npm run rust:test
cargo test --offline
```

桌面开发：

```sh
npm --workspace apps/desktop run dev
npm --workspace apps/desktop run tauri dev
npm --workspace apps/desktop run build
```

Node CLI 兼容入口：

```sh
node packages/skillbox-cli/bin/skillbox.js scan --json
node packages/skillbox-cli/bin/skillbox.js paths --json
node packages/skillbox-cli/bin/skillbox.js parse-github-url <github-url> --json
node packages/skillbox-cli/bin/skillbox.js install <github-url> --managed-root <temp-SkillBox> --json
```

Rust CLI 目标入口：

```sh
cargo run -p skillbox-cli --offline -- scan ~/.codex/skills ~/.agents/skills
cargo run -p skillbox-cli --offline -- paths
cargo run -p skillbox-cli --offline -- parse-github-url <github-url>
cargo run -p skillbox-cli --offline -- import <source-dir> --type user --managed-root <temp-SkillBox>
cargo run -p skillbox-cli --offline -- deploy <skill-name> --target <target-root> --managed-root <temp-SkillBox>
```

## 代码约定

- 核心业务逻辑放在 Rust crates，优先放入 `crates/skillbox-core`、`skillbox-github` 或 `skillbox-git`。
- React UI 只负责展示、交互状态和调用 Tauri commands。
- Tauri command 只做参数接收、类型转换和调用 Rust core，不承载复杂业务流程。
- 不要让 UI 直接调用 Git、文件系统、GitHub 下载或迁移逻辑。
- 不要执行用户提供的 shell 字符串；外部命令必须使用结构化参数。
- 路径写入前要展开 `~`、校验 skill name，并尽量用规范化后的路径比较。
- 远程 URL、下载内容、已有 runtime 目录都按不可信输入处理。
- 不要静默覆盖非 symlink runtime target。
- 涉及用户创建 skill 的 destructive 操作必须由用户明确确认。
- Node CLI 是 legacy transition layer；新增核心能力应进入 Rust，再按需补 Node 兼容包装。
- 新增 Claude、OpenClaw、Cursor、Claude Code、Copilot 等支持时，先在 Rust core 定义 agent adapter；不要把 agent-specific 目录和格式逻辑写进 React UI。

## 提交流程

分支：

- Codex 工作默认使用 `codex/` 前缀分支，除非用户指定其它分支名。
- 如果当前在 Codex 或其它 agent 管理的 worktree 中，先确认工作区状态，再编辑。

PR 前检查：

- 跑相关自动化测试：通常至少 `npm test` 和 `cargo test --offline`。
- 如果改动 UI，运行 Vite 或 Tauri dev，并做浏览器或 Tauri 手动验证。
- 如果改动 workflow，更新 `docs/workflows.md` 中对应完成标准。
- 如果改动存储、schema、目录布局或迁移行为，更新 `docs/data-model.md`。
- 如果推翻或新增长期架构选择，新增或更新 `docs/decisions/` ADR。
- 检查没有引入静默覆盖、直接 shell 字符串、未备份迁移或用户内容丢失风险。

## 常见坑

- `cargo` 不在 `PATH`：运行 `source ~/.cargo/env` 或使用完整路径。
- Browser preview 不是 Tauri：`apps/desktop` 在普通 Vite 预览中会使用 mock 状态，不能验证真实 Tauri command。
- Rust CLI 和 Node CLI 能力不完全相同：GitHub install、check updates、rollback、user-skills Git sync 仍在 Node 侧。
- Rust SQLite schema 和 Node MVP schema 有差异：改 schema 时必须考虑旧数据读取。
- `~/.codex/skills`、`~/.agents/skills` 和其它 agent runtime 都不是真相源，不能把 runtime 目录当作唯一状态。
