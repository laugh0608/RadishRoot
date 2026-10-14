# 当前状态

更新日期：2026-10-14（Asia/Shanghai）。

## 当前结论

RadishRoot 已建立产品文档、通用仓库治理基线及远端保护。产品文档仍为 2.0：第一 MVP、M0–M5、32 项任务和 24 项验收未变。T01 已开始环境准备，完成本机只读盘点与安装候选文档，尚未完成可编译工作区；T02–T32 未开始。没有 Cargo 工程、内核、镜像或产品运行入口；不能启动系统，没有业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 本地治理检查 | 本批 31 个文件的仓库检查通过；检查器既有 13 项回归通过证据见月度日志，本批未改检查器、不重复运行；两份根入口逐字一致 |
| T01 环境 | 稳定版 Rust 1.96.0 / 1.97.1 已有，裸机 target / nightly / 独立 QEMU CLI 缺失；UTM 内有固件样本；现有 OrbStack Docker 服务只读查询成功 |
| 安装候选 | Debian 日期标签 → ARM64 digest 已核对；QEMU 11.1.2 的 30 项制品候选身份已记录，选定 libssh 0.12.2 / OpenSSL 3.6.4_1；30 项配方来源 / 哈希和静态依赖图已核对；Debian 快照签名数学验证及五包可用性通过；实际包管理器事务 / 运行未验证，见[环境清单](../development/environment.md) |
| 安装影响 | 固定 QEMU 集合为 22 项新增、4 项升级、4 项同版本制品待核对；Debian 无 recommends 方案预计新增 5 包；固定 tap 与现有 core keg 的来源冲突待解决，数量仍为静态差异，未安装 |
| Git / 远端 | 已连接 [laugh0608/RadishRoot](https://github.com/laugh0608/RadishRoot)；master 为稳定主线，dev 为日常开发分支；治理晋级与回流的具体提交及检查证据保存在对应 PR |
| 默认分支与合并 | 默认 master；允许 merge / rebase，禁止 squash；关闭自动合并与自动删除 head branch |
| master Ruleset | [25073496](https://github.com/laugh0608/RadishRoot/rules/25073496) 为 active，仅覆盖 master；要求 PR、解决讨论、严格最新的 Candidate Quality；禁止删除 / 强推；0 审批，管理员仅 PR 内 bypass |
| 检查来源与权限 | Candidate Quality 绑定实测 GitHub Actions App 15368；工作流默认只读，不能审批 PR |
| dev 保护 | 有效 Rulesets 为空，传统保护返回未保护；允许按风险本地验证后直接提交 / push |
| 实际 Actions | 手动首跑成功；PR 故障候选阻断，修复候选通过；普通 dev push 不触发自动 CI |
| 产品验证 | 已读取工具版本；项目 Rust 编译 / 宿主交互 / QEMU guest / 读盘 / 安装均未执行；T01、AT01、AT23 均未通过 |

## 验证入口与证据

- 本地：`python3 scripts/check-repo.py`；回归：`python3 -B -m unittest discover -s scripts/tests -v`。PR 使用 `--base-ref` 检查提交范围。
- [master 手动首跑](https://github.com/laugh0608/RadishRoot/actions/runs/38557500889)：Repo Hygiene 与 Candidate Quality 均成功，核对运行 SHA 和 Check Run App ID。
- [验证 PR #1](https://github.com/laugh0608/RadishRoot/pull/1)：[故障运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557855601)两组件失败、PR blocked；[修复运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557986984)两组件通过、PR clean。PR 已关闭且未合并，临时验证分支已删除。
- 已读回默认分支、合并选项、Ruleset 详情、master / dev 有效规则及 Actions 权限。未对真实 master 做删除、直接 push、force push 或 bypass 演练。
- [月度日志](../devlogs/2026-10.md)记录配置、失败 / 成功证据、Git 动作和限制；[Ruleset 手册](../../.github/rulesets/README.md)维护操作流程。

治理证据只证明仓库门禁；环境盘点只证明已观察到的工具与元数据。现有 checkout pin 在 runner 上产生 Node.js 20 被强制转为 Node.js 24 的兼容提示，检查已成功；后续工具维护时专项评估。安全报告私密入口未启用或验收，仍按 SECURITY 指引处理。

## 下一项与停止线

继续在 dev。前批 registry / 冻结方案已提交并推送为 dc2b573；本批获授权只读核对 30 项 formula 来源 / 哈希 / 依赖解析及 Debian snapshot 签名 / 包可用性，同步文档、验证后提交并推送 dev。不直接推送 master，不创建 PR、产品 Release 或部署。

下一步仍为安装前准备：先解决固定 formula 集合与已有 Homebrew core keg 的来源冲突，形成可审阅的具体执行与恢复方案；保留 30 项精确身份和 OpenSSL 3 链，不通过卸载共享依赖、改写收据或绕过校验消除冲突。Debian 已取得固定快照签名 / 索引和五包路径证据，但 apt / sqv 策略验证与模拟求解未运行；历史快照的 Valid-Until 已过期，按环境专题仅处理固定源有效期，不关闭签名检查。实际事务、同版本项复用及反向依赖影响仍需验证；安装须另获授权。Cargo workspace、业务代码与 guest 运行另按实际授权推进。

本轮停止线：不安装 / 更新依赖，不拉取镜像层，不启动容器 / VM，不写业务代码或生成工具链锁文件。无本任务后台资源。原创许可、实际编译依赖与功能开关、Limine 协议配对、Live 来源及性能预算尚未冻结；实施排期前记录每周投入，94 人日仍是估算。T01 完成须有干净目录的最小 no_std 编译与依赖隔离证据，M0 退出还须 T02–T05 的真实存储证据。
