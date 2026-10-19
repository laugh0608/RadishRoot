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

**今日停在环境核验与 T01 方案完成的位置。** Rust 核验已提交为 `857b9e4`，T01 实施方案已提交为 `1110cb3`；均在本地 dev，未推送。本轮仅整理文档交接，不开始实现。工具就绪不等于可编译工作区，T01、M0 和产品 AT 仍未完成。

下次建议按以下顺序推进，具体文件职责与通过条件统一见 [T01 实施方案](../mvp-plan.md#t01-具体实施方案待实现)：

1. **先建立最小工作区与正确工具链选择。** 在获得实施授权后，仅建立 kernel / 宿主 xtask 两个成员及必要配置，暂不引入外部 crate。固定 nightly-2026-10-13；本机默认及 RUSTUP_TOOLCHAIN 仍为 1.96.0，入口和子命令必须选中指定 nightly，避免误用稳定版。
2. **取得第一份真实裸机产物。** 用已装 x86_64-unknown-none 和 rust-lld 完成 dev / release 干净链接，核对 x86_64 ELF 的入口、段权限、布局和宿主依赖隔离，并保留独立读取证据。高半区链接地址不代表分页、物理装载或启动已经实现。
3. **补齐检查、失败路径与 CI。** 实现 check、build、test --host 的实际行为及超时回收；加入配置冲突、损坏 ELF 和子进程失败测试。同步 rust-checks、Candidate Quality 的 needs、检查器精确基线及拒绝回归，不仅修改工作流。
4. **按证据收口 T01，再进入 T02。** 本地验证与实际 Actions 分开记录；未执行的远端检查保持未验证。T02 才开始 Limine 配对、引导与串口；存储、重启及独立读盘继续按 T03–T05 推进。

**当前停止线：**今天不再创建工程或业务代码，不追加依赖、不修改 CI 实现、不启动 guest、不推送或执行其他远端操作。下次实施前只需核对工作区、工具链选择与方案是否有变化；没有变化时直接沿现有环境证据推进，不重复全套安装核验。原创许可和新增依赖在实际引入 / 发行节点核对，排期前确认投入时间。

**保留材料与资源：**Rust 核验证据约 16 MiB、方案元数据约 924 KiB、QEMU 核验与预演各约 40 MiB、GPT/FAT 稀疏材料约 50 MiB 及原 Docker 工具镜像继续保留；精确位置和身份见[环境清单](../development/environment.md)及[本月日志](../devlogs/2026-10.md)。本次没有新增后台资源，也未清理用户或其他任务资源。
