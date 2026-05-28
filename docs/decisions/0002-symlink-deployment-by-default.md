# ADR 0002: 默认使用 Symlink 部署

## 背景

SkillBox 需要把 managed store 中的 skill、规则、提示词或能力包暴露给不同 agent 的 runtime 目录。可选方案包括复制快照、移动目录、生成 agent 原生文件，或创建 symlink。

## 决定

对目录型 skill 或能力包，默认部署方式是从 runtime 目录创建 symlink，指向 `~/.skillbox` 中的 managed skill。
对不支持目录 symlink 语义的 agent，必须由 adapter 明确声明其它部署方式。

## 理由

- Symlink 让 runtime 始终看到 managed store 的当前内容。
- user skill 编辑后不需要重复复制。
- remote skill 更新或 rollback 只需要切换 managed `current` 指针和 runtime symlink。
- Runtime 目录中已有的非 symlink 内容可以被明确识别并拒绝覆盖。
- 对 Cursor、Copilot 等可能使用单文件或项目规则文件的 agent，adapter 可以选择生成文件或 copy snapshot，但必须保留同等冲突保护。

## 后果

- 部署逻辑必须检查 target 是否存在。
- target 是非 symlink 时必须拒绝。
- target 是 symlink 但指向其它位置时必须拒绝。
- 导入并替换原 runtime 目录时，必须先把原目录移动到 backup，再创建 symlink。
- 后续可以增加复制快照或生成型部署模式，但不能改变目录型部署中 symlink 作为默认模式的安全语义。
