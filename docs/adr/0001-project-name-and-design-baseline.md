# ADR 0001：项目名称与设计基线

- 日期：2026-10-07
- 状态：Accepted

## 背景

用户已创建 https://github.com/F31/nai.git，并确定英文名称为 `nai`。
V3.5 文档使用建议名称 `digital-host-agent` 及示意工程前缀 `dh`。
当前工作目录仅有设计资料，无既有应用包需要兼容。

## 决策

1. 产品、仓库、README 与应用展示名称统一为 `nai`。
2. 采用 `docs/digital-host-agent-design-V3.5-OpenCode.md` 为唯一实现基线，
   `docs/design.md` 仅链接基线；保留原始设计和历史资料。
3. 新建 Rust 包将示意 `dh-*` 映射为 `nai-*`，例如 `nai-contracts`、
   `nai-kernel`、`nai-xtask`；TypeScript 包使用 `@nai/*`。
   Rust 源码引用相应使用 `nai_contracts` 等下划线形式。
4. `cargo xtask` 仍为对外任务命令，其 alias 将指向 `nai-xtask`；
   前端命令过滤器相应使用 `@nai/desktop`、`@nai/remote-web`。
5. 能力名如 `platform.events@1`、`guard.check@1`、`avatar.render@1`、
   `session_id` 和 `generation_epoch` 保持规范语义，不随产品改名。

## 影响与验证

后续 D01/D02 依据此映射创建工程，不改动 V3.5 的技术与验收要求。
创建 workspace 时验证 Cargo 包名、依赖、xtask alias、pnpm 过滤器和 CI 一致。
本 ADR 不代表上述包或命令已经实现。
