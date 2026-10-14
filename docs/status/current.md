# 当前状态

更新日期：2026-10-14（Asia/Shanghai）。

## 当前结论

RadishRoot 已建立产品文档、通用仓库治理基线及远端保护。产品文档仍为 2.0：第一 MVP、M0–M5、32 项任务和 24 项验收未变，业务工程任务全部未开始。没有 Cargo 工程、内核、镜像或产品运行入口；不能启动系统，没有业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 本地治理检查 | 30 个正式文件检查、13 项回归通过；两份根入口逐字一致 |
| Git / 远端 | 已连接 [laugh0608/RadishRoot](https://github.com/laugh0608/RadishRoot)；master 保留首次基线 8e6b019，本轮治理更新在 dev 维护；日常继续 dev |
| 默认分支与合并 | 默认 master；允许 merge / rebase，禁止 squash；关闭自动合并与自动删除 head branch |
| master Ruleset | [25073496](https://github.com/laugh0608/RadishRoot/rules/25073496) 为 active，仅覆盖 master；要求 PR、解决讨论、严格最新的 Candidate Quality；禁止删除 / 强推；0 审批，管理员仅 PR 内 bypass |
| 检查来源与权限 | Candidate Quality 绑定实测 GitHub Actions App 15368；工作流默认只读，不能审批 PR |
| dev 保护 | 有效 Rulesets 为空，传统保护返回未保护；允许按风险本地验证后直接提交 / push |
| 实际 Actions | 手动首跑成功；PR 故障候选阻断，修复候选通过；普通 dev push 不触发自动 CI |
| 产品验证 | Rust / 宿主交互 / QEMU / 读盘 / 安装均未执行 |

## 验证入口与证据

- 本地：`python3 scripts/check-repo.py`；回归：`python3 -B -m unittest discover -s scripts/tests -v`。PR 使用 `--base-ref` 检查提交范围。
- [master 手动首跑](https://github.com/laugh0608/RadishRoot/actions/runs/38557500889)：Repo Hygiene 与 Candidate Quality 均成功，核对运行 SHA 和 Check Run App ID。
- [验证 PR #1](https://github.com/laugh0608/RadishRoot/pull/1)：[故障运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557855601)两组件失败、PR blocked；[修复运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557986984)两组件通过、PR clean。PR 已关闭且未合并，临时验证分支已删除。
- 已读回默认分支、合并选项、Ruleset 详情、master / dev 有效规则及 Actions 权限。未对真实 master 做删除、直接 push、force push 或 bypass 演练。
- [月度日志](../devlogs/2026-10.md)记录配置、失败 / 成功证据、Git 动作和限制；[Ruleset 手册](../../.github/rulesets/README.md)维护操作流程。

本轮只证明仓库治理与远端门禁。现有 checkout pin 在 runner 上产生 Node.js 20 被强制转为 Node.js 24 的兼容提示，检查已成功；后续工具维护时专项评估，不在本批顺带升级依赖。安全报告私密入口未在本批启用或验收，仍按 SECURITY 指引处理。

## 下一项与停止线

后续普通开发继续在 dev，阶段晋级通过 PR 到 master，合并后先回流 dev。本轮状态更新不直接提交 master，不创建产品 Release 或部署。

业务实施收到指令后从 T01 开始，只读盘点 Rust、QEMU、固件与镜像工具，列出版本、缺口和安装影响，再建立最小编译入口。T02–T05 先取得写回、重启与宿主独立读盘证据，随后 T06–T13 形成真实图形闭环。实施前记录每周投入；94 人日仅为估算，M0 / M1 后按实测重估。

本轮授权覆盖默认分支、PR 合并选项、Ruleset、必要验证和状态同步；此前“暂不连接远端”已由用户后续指令替换。原创许可、产品工具链 / 依赖版本、Live 来源与性能预算仍待确定；业务实现、依赖安装、VM 和发布未开始。
