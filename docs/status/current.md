# 当前状态

更新日期：2026-10-19（Asia/Shanghai）。

## 当前结论

RadishRoot 已建立产品文档、通用仓库治理基线及远端保护。产品文档仍为 2.0：第一 MVP、M0–M5、32 项任务和 24 项验收未变。T01 已开始环境准备，完成本机盘点、安装候选文档、Debian 签名策略验证、本地工具镜像构建及 GPT/FAT 普通文件操作验证，尚未完成可编译工作区；T02–T32 未开始。没有 Cargo 工程、内核、产品镜像或产品运行入口；不能启动系统，没有业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 本地治理检查 | 本批 36 文件仓库检查通过；检查器既有 13 项回归通过证据沿用，本批未改检查器、不重复运行；两份根入口逐字一致 |
| T01 环境 | 稳定版 Rust 1.96.0 / 1.97.1 已有，裸机 target / nightly / 独立 QEMU CLI 缺失；UTM 内有固件样本；已通过现有 OrbStack Docker 服务构建工具镜像，并验证 GPT/FAT 制盘、独立读回及五类损坏检测 |
| 安装候选 | Debian 日期标签 → ARM64 digest 已核对；QEMU 11.1.2 的 30 项制品候选身份已记录，选定 libssh 0.12.2 / OpenSSL 3.6.4_1；30 项配方来源 / 哈希和静态依赖图已核对；Debian sqv / APT 验证通过；派生工具镜像已安装五包并核对包体、83 包清单及 7 个 CLI 命令；QEMU 实际事务 / 运行未验证，见[环境清单](../development/environment.md) |
| 安装影响 | 拟采用专用 `/opt/rrt01`，QEMU 集合 30 项全部专用新增，0 项共享升级 / 重装；30 项元数据满足短前缀重定位条件，尚未实际安装；Debian 工具镜像实际新增 5 包、0 升级 / 移除；原基础 78 包版本未变 |
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

继续在 dev。本轮按授权完成已有 Debian 工具镜像的 GPT/FAT 普通文件验证；QEMU / nightly 安装与 Cargo 实现不在本批范围。验证阶段开始时 HEAD 为 `ee6bcff`，与本地 origin/dev 相差 0/0；文档提交前只读确认远端 dev 仍指向同一提交。

已使用不可变镜像 ID 制作 1 GiB / 512 字节扇区 GPT、128 MiB FAT32 ESP 与 DATA。四份合成文件的名称、长度和内容哈希通过宿主独立解析核对；写入前后 DATA 之外字节不变；五类损坏副本均被独立检查拒绝。sgdisk 对 GPT 损坏可返回 0，fsck.fat 也不能发现任意文件内容篡改，因此验收保留原始 CRC 与预期哈希校验。具体布局提示、工具结果和材料身份见[环境清单第 2.7 节](../development/environment.md)。这仅是工具验证，不是内核持久化或产品 AT 通过。

两个任务容器均退出并按完整 ID 删除，标签查询无残留；保留原镜像和忽略目录 `evidence/t01-disk-20261019-ee6bcff/` 中的稀疏测试盘、脚本及证据，实际占盘约 50 MiB，无本任务后台资源。未安装依赖、运行 guest 或写业务代码。本批 Git 交付范围仅为环境清单、current、本月日志三份文档，按用户后续授权提交并推送 dev；测试盘与本地证据不入库，不发布。实际提交及同步结果以 Git 读回为准。

后续顺序沿用[环境清单](../development/environment.md)，不另建版本或依赖清单：

1. **完成 QEMU 隔离环境。** 按第 2.4 节复核固定 Homebrew / portable Ruby、专用 `/opt/rrt01` 和固定 tap，取得真实 30 项解析、安装、重定位与恢复证据；核对共享环境未变、CLI / TCG / 机器型号、固件来源及哈希。出现版本或依赖差异时先解释差异，不自动切换移动源。
2. **完成 Rust 工具链与 T01 最小编译。** 按已记录候选安装指定 nightly / target / 组件，显式选择工具链；在实施授权内建立真实 Cargo workspace，冻结实际依赖和 features，并在干净输出目录取得 x86_64 最小 no_std 编译与宿主依赖隔离证据。T01 完成后再依 T02–T05 取得引导、写入、flush、重启与独立读盘证据；M0 退出条件不变。

当前停止线：本批工具文件验证已收口，不继续安装 QEMU / nightly、启动 guest、写业务代码或生成 Rust 工具链锁文件；后续按覆盖相应操作与影响的授权推进。原创许可、实际编译依赖与功能开关、Limine 协议配对、Live 来源及性能预算仍未冻结，在对应引入 / 发行节点核对；实施排期前记录每周投入，94 人日仍是估算。
