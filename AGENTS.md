# SkillBox Agent Guide

## 项目目标

SkillBox 是一个本地 macOS 应用和 CLI，用来管理主流 agent 可用的 skills、规则、提示词和能力包。
覆盖 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 agent 生态。

SkillBox 管理两类内容：

- 用户创建的 skills，存放在 `~/SkillBox/user-skills`。
- 远程下载或导入的 skills，存放在 `~/SkillBox/remote-skills`。

`~/SkillBox` 是 SkillBox 的真相源。各 agent 的 runtime 目录只应被当作部署目标，
例如 `~/.codex/skills`、`~/.agents/skills`、项目局部 runtime，或后续 adapter 支持的 Claude、Cursor、Copilot 等目录。

## 必守规则

- 业务逻辑优先放在 Rust crates 中，桌面 UI 通过 Tauri commands 调用核心能力。
- React 层不能直接拥有文件系统、Git、GitHub、下载、迁移或回滚行为。
- 新增核心业务逻辑不要继续扩展 legacy Node CLI；Node 代码只作为过渡层和兼容参考。
- 文件系统操作必须显式、可验证，并尽量具备备份或回滚路径。
- 不要执行用户提供的 shell 字符串；使用结构化参数和校验后的路径。
- GitHub URL、远程归档、外部路径和现有 runtime skills 都是不可信输入。
- 不要静默覆盖 runtime 目录中的既有 skill，尤其不能覆盖非 symlink 目标。
- 除非用户明确确认破坏性操作，否则必须保留用户创建的 skill 内容。
- 不要把某个 agent 的格式当成全局格式；跨 agent 行为必须通过 adapter 或明确的兼容层表达。

## 当前实现边界

- Rust 已覆盖 `SKILL.md` 目录的扫描、导入、候选导入、symlink 部署、SQLite 基础索引、GitHub URL 解析和 Git 状态读取。
- Tauri 桌面桥接当前调用 Rust commands，不再通过 UI 直接 shell 到 Node CLI。
- Node CLI 仍覆盖 GitHub install、check updates、remote rollback 和 user-skills Git sync。
- Claude、OpenClaw、Cursor、Claude Code、Copilot 等非 `SKILL.md` 或非 Codex-style runtime 的支持尚需 agent adapter 层。
- 目标方向是 CLI 和 UI 共享 Rust core；Node CLI 是 legacy transition layer。

## 文档导航

- 系统地图和模块边界：`docs/architecture.md`
- 存储布局、SQLite、命名和兼容规则：`docs/data-model.md`
- 可执行 workflow 和完成标准：`docs/workflows.md`
- 本地开发、测试和提交规范：`CONTRIBUTING.md`
- 架构决策记录：`docs/decisions/`
- 当前实现进度快照：`docs/implementation-status.md`

## 验证要求

每个有意义的改动都必须包含自动化测试，或给出清楚的手动验证记录。

在声称某个 workflow 完成之前，必须运行对应测试或命令，并报告验证内容。
需要保持可验证的 workflow 见 `docs/workflows.md`。
