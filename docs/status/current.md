# 当前状态

更新日期：2026-10-19（Asia/Shanghai）。

## 当前结论

RadishRoot 已建立产品文档、通用仓库治理基线及远端保护。产品文档仍为 2.0：第一 MVP、M0–M5、32 项任务和 24 项验收未变。T01 已开始环境准备，完成本机盘点、安装候选文档、Debian 签名策略验证、本地工具镜像构建、GPT/FAT 普通文件操作验证、正常 Homebrew QEMU 与指定 Rust nightly 安装后核验，尚未完成可编译工作区；T02–T32 未开始。没有 Cargo 工程、内核、产品镜像或产品运行入口；不能启动系统，没有业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 本地治理检查 | 本批 36 文件仓库检查通过；检查器既有 13 项回归通过证据沿用，本批未改检查器、不重复运行；两份根入口逐字一致 |
| T01 环境 | 稳定版 Rust 1.96.0 / 1.97.1 保留；nightly-2026-10-13、七项组件及 x86_64-unknown-none 已安装并核验；QEMU 11.1.2、Debian 工具镜像与 GPT/FAT 工具验证已有证据 |
| Rust 实测 | rustc / Cargo 1.101.0-nightly，LLVM / rust-lld 23.1.3；裸机库文件与目标配置已核对，链接器 GNU 模式查询通过。默认仍为 1.96.0，须显式选择 nightly；未执行项目编译，见[环境清单第 2.9 节](../development/environment.md) |
| QEMU 实测 | 30 项活动版本与候选一致；支持 TCG、pc-q35-11.1，显示后端为 none / curses / cocoa / dbus；x86_64 固件 descriptor 与实体 / 哈希已核对。38 包无缺失库，QEMU 实际加载 libssh → OpenSSL 4，见[环境清单第 2.8 节](../development/environment.md) |
| 共享环境 | 默认 OpenSSL 为 4.0.3，3.6.4 专用路径仍可用；相对安装前新增 23 包和 4 个升级版本，原 40 份收据保留且未改写；OpenSSL 3 / 4 共用 CA。相关项目功能兼容性及 TLS 行为尚未验证 |
| Git / 远端 | 已连接 [laugh0608/RadishRoot](https://github.com/laugh0608/RadishRoot)；master 为稳定主线，dev 为日常开发分支；治理晋级与回流的具体提交及检查证据保存在对应 PR |
| 默认分支与合并 | 默认 master；允许 merge / rebase，禁止 squash；关闭自动合并与自动删除 head branch |
| master Ruleset | [25073496](https://github.com/laugh0608/RadishRoot/rules/25073496) 为 active，仅覆盖 master；要求 PR、解决讨论、严格最新的 Candidate Quality；禁止删除 / 强推；0 审批，管理员仅 PR 内 bypass |
| 检查来源与权限 | Candidate Quality 绑定实测 GitHub Actions App 15368；工作流默认只读，不能审批 PR |
| dev 保护 | 有效 Rulesets 为空，传统保护返回未保护；允许按风险本地验证后直接提交 / push |
| 实际 Actions | 手动首跑成功；PR 故障候选阻断，修复候选通过；普通 dev push 不触发自动 CI |
| 产品验证 | 已读取工具版本；项目 Rust 编译 / 宿主交互 / QEMU guest / 内核写回后的独立读盘 / 产品安装均未执行；T01、AT01、AT23 均未通过 |

## 验证入口与证据

- 本地：`python3 scripts/check-repo.py`；回归：`python3 -B -m unittest discover -s scripts/tests -v`。PR 使用 `--base-ref` 检查提交范围。
- [master 手动首跑](https://github.com/laugh0608/RadishRoot/actions/runs/38557500889)：Repo Hygiene 与 Candidate Quality 均成功，核对运行 SHA 和 Check Run App ID。
- [验证 PR #1](https://github.com/laugh0608/RadishRoot/pull/1)：[故障运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557855601)两组件失败、PR blocked；[修复运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557986984)两组件通过、PR clean。PR 已关闭且未合并，临时验证分支已删除。
- 已读回默认分支、合并选项、Ruleset 详情、master / dev 有效规则及 Actions 权限。未对真实 master 做删除、直接 push、force push 或 bypass 演练。
- [月度日志](../devlogs/2026-10.md)记录配置、失败 / 成功证据、Git 动作和限制；[Ruleset 手册](../../.github/rulesets/README.md)维护操作流程。

治理证据只证明仓库门禁；环境盘点只证明已观察到的工具与元数据。现有 checkout pin 在 runner 上产生 Node.js 20 被强制转为 Node.js 24 的兼容提示，检查已成功；后续工具维护时专项评估。安全报告私密入口未启用或验收，仍按 SECURITY 指引处理。

## 接下来的事项与停止线

继续在 dev。Rust 核验四份文档已按授权提交为 `857b9e41851daa2ad30925a24cdacf515de1c26e`；连同此前 QEMU 方案与核验提交，本地领先 origin/dev 3 个提交，均未推送，未刷新远端引用。提交后工作区干净；随后仅梳理 T01 实施方案，修改 MVP 对应条目、current 和本月日志，尚未提交这部分规划。本轮没有创建 Cargo 工程或写业务代码。

Rust 安装后核验通过：实际版本符合指定 nightly，七项组件及宿主 / 裸机 target 齐全，目标库为 x86_64 ELF 对象；rust-lld 为原生 arm64 可执行文件，GNU 模式查询及同工具链 libLLVM 加载通过。默认与环境变量仍选择 1.96.0，后续须显式选择 nightly。QEMU 使用 pc-q35-11.1 / TCG 候选与已记录的配对固件；两项工具核验均不替代最小编译或 guest 启动。

Rust 核验前后 settings 和三套工具链的 35 份配置 / 组件元数据摘要一致，未执行安装、全局配置修改或编译。忽略目录 `evidence/t01-rust-20261019-bac6c12/` 保留约 16 MiB 证据 / 许可；此前 Rust 方案元数据约 924 KiB、QEMU 核验与预演各约 40 MiB、GPT/FAT 稀疏材料约 50 MiB 及原 Docker 镜像继续保留。没有新增后台资源；本轮未重新运行安装后工具核验或新增工具下载。

下一步是 **按 [MVP 的 T01 实施方案](../mvp-plan.md#t01-建立可编译工作区与环境清单)建立最小可编译工作区**，本轮仅完成方案梳理。规划用 kernel / 宿主 xtask 两个成员起步，暂不引入外部 crate；固定项目工具链并隔离宿主与裸机配置，落实实际 check、build、test --host 三个入口。验收包括新输出目录的 dev / release ELF 链接、独立头 / 段结构核对、配置与依赖隔离、失败用例，以及 CI 与治理检查器同步。

现有 CI 检查器精确限定两个 job；实施时须连同 rust-checks、聚合 needs、超时基线与拒绝测试一起更新。当前没有执行 Rust 构建、测试或新增 CI，T01 仍未完成；不能把计划入口或已安装工具记为可编译工作区。T02–T05 的引导、串口、存储、重启和独立读盘仍按原边界推进，M0 / AT 条件不变。

当前停止线：本轮规划结束后，不创建 Cargo / 工具链配置、写内核或 xtask 代码、追加依赖、修改 CI 实现、启动 guest、推送或开启远端操作。原创许可、实际依赖和产品运行参数在对应引入 / 发行节点核对；规划中的 ELF 虚拟链接基址不代表分页 / 物理装载已实现。实施排期前记录每周投入，94 人日仍是估算。
