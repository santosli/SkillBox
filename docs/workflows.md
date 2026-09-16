# SkillBox Workflows

本文件定义工作流入口、步骤、失败处理和完成标准。实现位置和长期目标见 `docs/architecture.md`。
SkillBox 的目标是跨 agent 管理，不只覆盖 Codex。当前 workflow 以 Rust runtime
profiles 管理 `SKILL.md` roots；
Claude、OpenClaw、Cursor、Claude Code、Copilot 等需要通过 agent adapter 扩展。

## 1. Scan Local Skill Roots

触发条件：

- UI 刷新 managed state 或扫描 import candidates。
- Rust CLI 执行 `scan`。

步骤：

- 从 versioned Rust profile registry 按 precedence 读取
  `~/.agents/skills`、`~/.codex/skills`、`~/.claude/skills`、
  `~/.cursor/skills` 及对应项目局部 roots。
- 后续通过 agent adapter 读取 Claude、OpenClaw、Cursor、Claude Code、Copilot 等 runtime roots。
- 在每个 root 内递归查找包含 `SKILL.md` 的目录。
- 扫描摘要读取 frontmatter 的 `name`、`description`、`version`；严格的完整
  structured parse 在 deployment compatibility preview 执行。
- 计算 `SKILL.md` content hash。
- 标记 source root、是否 symlink、real path。
- 扫描 import candidates 时把存在且可读取的 skills root 写入 `workspaces` registry；home-level roots 记为 `global`，项目局部 roots 记为 `user`。
- 按 skill name 排序返回，同时保留 scan errors。

失败与回滚：

- 不存在的 root 跳过。
- 单个 skill 读取失败时记录 error，不中断整个扫描。
- scan 不应写入 runtime 目录，因此不需要回滚。

完成验证：

- `cargo test --offline`
- `npm test`
- `cargo run -p skillbox-cli --offline -- scan ~/.codex/skills ~/.agents/skills`
