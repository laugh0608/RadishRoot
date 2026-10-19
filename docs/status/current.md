# 当前状态

更新日期：2026-10-19（Asia/Shanghai）。

## 当前结论

RadishRoot 已建立产品文档、通用仓库治理基线及远端保护。产品文档仍为 2.0：第一 MVP、M0–M5、32 项任务和 24 项验收未变。T01 已开始环境准备，完成本机盘点、安装候选文档、Debian 签名策略验证、本地工具镜像构建及 GPT/FAT 普通文件操作验证，尚未完成可编译工作区；T02–T32 未开始。没有 Cargo 工程、内核、产品镜像或产品运行入口；不能启动系统，没有业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 本地治理检查 | 本批 36 文件仓库检查通过；检查器既有 13 项回归通过证据沿用，本批未改检查器、不重复运行；两份根入口逐字一致 |
| T01 环境 | 稳定版 Rust 1.96.0 / 1.97.1 已有，裸机 target / nightly / 独立 QEMU CLI 缺失；UTM 内有固件样本；已通过现有 OrbStack Docker 服务构建工具镜像，并验证 GPT/FAT 制盘、独立读回及五类损坏检测 |
| 安装候选 | 2026-10-19 已改为正常 Homebrew QEMU 11.1.2，当前 libssh 0.12.2_1 依赖 OpenSSL 4.0.3；官方元数据与实际 dry-run 一致。Debian 签名 / 包体 / 83 包清单与 CLI 验证已通过；QEMU 尚未安装，详见[环境清单](../development/environment.md) |
| 安装影响 | 共享 `/opt/homebrew` 预演为 23 项新增、4 项升级、3 项复用；会改变默认 OpenSSL CLI / 编译发现及共享 CA，影响 Git、ripgrep 和 RadishLex 的相关本机路径；尚无升级后兼容性证据。专用前缀 / 固定 tap 方案已撤换 |
| Git / 远端 | 已连接 [laugh0608/RadishRoot](https://github.com/laugh0608/RadishRoot)；master 为稳定主线，dev 为日常开发分支；治理晋级与回流的具体提交及检查证据保存在对应 PR |
| 默认分支与合并 | 默认 master；允许 merge / rebase，禁止 squash；关闭自动合并与自动删除 head branch |
| master Ruleset | [25073496](https://github.com/laugh0608/RadishRoot/rules/25073496) 为 active，仅覆盖 master；要求 PR、解决讨论、严格最新的 Candidate Quality；禁止删除 / 强推；0 审批，管理员仅 PR 内 bypass |
| 检查来源与权限 | Candidate Quality 绑定实测 GitHub Actions App 15368；工作流默认只读，不能审批 PR |
| dev 保护 | 有效 Rulesets 为空，传统保护返回未保护；允许按风险本地验证后直接提交 / push |
| 实际 Actions | 手动首跑成功；PR 故障候选阻断，修复候选通过；普通 dev push 不触发自动 CI |
| 产品验证 | 已读取工具版本；项目 Rust 编译 / 宿主交互 / QEMU guest / 内核写回后的独立读盘 / 安装均未执行；T01、AT01、AT23 均未通过 |

## 验证入口与证据

- 本地：`python3 scripts/check-repo.py`；回归：`python3 -B -m unittest discover -s scripts/tests -v`。PR 使用 `--base-ref` 检查提交范围。
- [master 手动首跑](https://github.com/laugh0608/RadishRoot/actions/runs/38557500889)：Repo Hygiene 与 Candidate Quality 均成功，核对运行 SHA 和 Check Run App ID。
- [验证 PR #1](https://github.com/laugh0608/RadishRoot/pull/1)：[故障运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557855601)两组件失败、PR blocked；[修复运行](https://github.com/laugh0608/RadishRoot/actions/runs/38557986984)两组件通过、PR clean。PR 已关闭且未合并，临时验证分支已删除。
- 已读回默认分支、合并选项、Ruleset 详情、master / dev 有效规则及 Actions 权限。未对真实 master 做删除、直接 push、force push 或 bypass 演练。
- [月度日志](../devlogs/2026-10.md)记录配置、失败 / 成功证据、Git 动作和限制；[Ruleset 手册](../../.github/rulesets/README.md)维护操作流程。

治理证据只证明仓库门禁；环境盘点只证明已观察到的工具与元数据。现有 checkout pin 在 runner 上产生 Node.js 20 被强制转为 Node.js 24 的兼容提示，检查已成功；后续工具维护时专项评估。安全报告私密入口未启用或验收，仍按 SECURITY 指引处理。

## 接下来的事项与停止线

继续在 dev。本批按用户要求只核对正常 Homebrew QEMU 的当前版本、依赖动作及其他项目影响，并更新方案；明确暂不安装。开始时 HEAD / 本地 origin/dev 均为 `19ad0a57e92863e175fb2884c0471662e7fa230a`，工作树、暂存及未跟踪状态干净；未刷新远端引用。本批只修改环境清单、current、本月日志；用户随后授权提交这三份文档，未授权推送或安装，具体提交结果以 Git 读回为准。

已执行正常 Homebrew dry-run，退出 0；计划为 QEMU + 22 项新增依赖，升级 libunistring、pcre2、ca-certificates、snappy，复用 gettext、libyaml、xz。完整版本 / 哈希及共享影响见[环境清单第 2.2–2.4 节](../development/environment.md)。特别是 openssl@4 配方允许覆盖现有 OpenSSL 3 默认链接，ca-certificates 安装后会读取系统证书并更新共享 CA；保留 openssl@3 不等于其他项目不受影响。RadishLex 的共享 librime 和未限定版本的 openssl 调用路径已只读核对，未运行或修改兄弟项目。

本次预演前后 40 份安装收据、424 条共享链接和 Homebrew commit 一致，QEMU 仍未安装。公开元数据、输出与基线保留在忽略目录 `evidence/t01-homebrew-20261019-19ad0a5/`。先前 GPT/FAT 工具验证及容器清理已收口，原镜像与 `evidence/t01-disk-20261019-ee6bcff/` 中约 50 MiB 稀疏材料继续保留，详见环境清单第 2.7 节；无本任务后台资源。

后续顺序沿用环境清单，不另建版本或依赖清单：

1. **完成正常 Homebrew QEMU 环境。** 在覆盖共享依赖升级、默认 OpenSSL 切换及 CA 更新影响的安装授权内，重新预演后安装；核对真实依赖 / 链接、相关共享工具、CLI / TCG / 机器型号及固件来源 / 哈希。若范围变化先报告，不把预演成功当作运行兼容或可靠回滚证明。
2. **完成 Rust 工具链与 T01 最小编译。** 按已记录候选安装指定 nightly / target / 组件，显式选择工具链；在实施授权内建立真实 Cargo workspace，冻结实际依赖和 features，并在干净输出目录取得 x86_64 最小 no_std 编译与宿主依赖隔离证据。T01 完成后再依 T02–T05 取得引导、写入、flush、重启与独立读盘证据；M0 退出条件不变。

当前停止线：本批方案核对完成后不继续安装 QEMU / nightly、改变共享链接 / 证书、启动 guest、写业务代码或生成 Rust 工具链锁文件；后续按覆盖相应操作与影响的授权推进。原创许可、实际编译依赖与功能开关、Limine 协议配对、Live 来源及性能预算仍未冻结，在对应引入 / 发行节点核对；实施排期前记录每周投入，94 人日仍是估算。
