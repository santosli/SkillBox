# ADR 0004: 通过 Adapter 支持多 Agent Runtime

## 背景

SkillBox 面向多 agent runtime。目标用户会同时使用 Claude、Codex、OpenClaw、Cursor、Claude Code、Copilot 等 agent，而这些工具的目录布局、原生格式、部署语义和冲突风险不同。

## 决定

SkillBox 的 managed store 保持 agent-agnostic。每个 agent runtime 通过 adapter 接入：

- adapter 负责发现 runtime roots。
- adapter 负责读取该 agent 的原生格式。
- adapter 负责把原生内容映射到 SkillBox 的规范化记录。
- adapter 负责部署回该 agent 的 runtime，并声明 symlink、copy snapshot 或生成文件等模式。

## 理由

- 单一 `SKILL.md` 目录模型无法表达所有主流 agent 的能力格式。
- UI 不应该知道 Claude、Cursor、Copilot 等工具的目录细节。
- adapter 边界能让扫描、导入、部署和冲突处理继续在 Rust core 中测试。
- managed store 作为真相源，可以避免不同 agent runtime 互相覆盖。

## 后果

- 当前 `.codex/.agents` 支持只是第一阶段，不代表最终格式边界。
- 新增 agent 支持时，必须同时更新 architecture、data model、workflow 和测试。
- adapter 可以选择非 symlink 部署，但必须提供与 symlink 默认模式同级别的冲突保护和回滚路径。
- SQLite schema 需要迁移以表达 `agent_id`、runtime target 和 format 信息。
