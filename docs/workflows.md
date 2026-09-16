# SkillBox Workflows

本文件定义工作流入口、步骤、失败处理和完成标准。实现位置和长期目标见 `docs/architecture.md`。
SkillBox 的目标是跨 agent 管理，不只覆盖 Codex。当前 workflow 以 Rust runtime
profiles 管理 `SKILL.md` roots；
Claude、OpenClaw、Cursor、Claude Code、Copilot 等需要通过 agent adapter 扩展。
