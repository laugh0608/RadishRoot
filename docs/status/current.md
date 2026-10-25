# 当前状态

更新日期：2026-10-25（Asia/Shanghai）。

## 当前结论

**T01 本地实现与必要验证通过，下一项为 T02 引导、串口与最小故障停止。** 产品文档仍为 2.0，第一 MVP、M0–M5、32 项任务和 24 项验收未变。已建立 kernel / xtask 两成员 Cargo workspace，取得两种 profile 的真实静态 ELF；新增 CI 配置已同步，但尚未运行远端 Actions。T02–T32 未开始，M0、AT01、AT23 未通过；没有可启动的产品镜像、guest 运行入口或业务验收证据。

| 层级 | 实际状态 |
| --- | --- |
| 工程与工具链 | 版本由 workspace.package.version 单一提供（26.10.0-dev）；nightly-2026-10-13、x86_64-unknown-none；锁文件离线生成，仅两个本地包，无第三方 crate；全局默认仍为 1.96.0 |
| 本地 Rust 验证 | fmt、宿主 / 裸机 check 与 clippy（拒绝警告）、13 项宿主测试通过；覆盖参数、工具链 / 配置拒绝、ELF 损坏、子进程非零 / 信号 / 超时及输出边界 |
| 真实链接 | dev / release 各用新输出目录完成链接；独立 Python struct、系统 file、shasum 与 linker map 核对通过；两者均为 x86_64 ET_EXEC，entry=_start=0xffffffff80000000，无动态段 / RWX |
| 实际入口拒绝 | 错误活动工具链、冲突 RUSTFLAGS、未知命令、非法超时及本地 CI install 拒绝通过；配置拒绝发生在 xtask 启动 Cargo 子命令前 |
| 仓库 / CI | 本地仓库检查、14 项检查器回归通过；根入口逐字一致。新增 rust-checks，聚合要求精确两组件全部 success，超时 5 / 15 / 2 分钟；新增 Linux job 的实际运行未验证 |
| 既有环境 | QEMU 11.1.2 / pc-q35-11.1 / TCG、固件及 Debian GPT/FAT 工具沿用既有核验证据；本轮没有重新安装、启动 QEMU 或容器 |
| 产品能力 | 仅编译与宿主工具，不代表分页、物理装载、引导、存储写回、图形交互或安装通过 |
| Git | dev，HEAD=e924ee1；本批源码 / 配置 / 文档留在工作区，未暂存、提交或推送。与本地 origin/dev 的提交差为 0 / 0，本轮未 fetch，不能据此声明远端实时同步 |

## 可执行入口与证据

- 本地使用方式见[开发规范的 T01 入口](../development/standards.md#t01-本地入口)，实施契约见 [MVP T01](../mvp-plan.md#t01-实施契约与验证入口)。`check`、`build --profile dev|release [--fresh]`、`test --host` 已实际执行；其他产品入口仍未实现。
- 本批证据：忽略目录 `evidence/t01-workspace-20261025-e924ee1/`，含独立 ELF 复核脚本与结果、入口拒绝结果、源码内容摘要、日志 / 产物副本和 SHA256SUMS。候选由 HEAD 与未提交源码摘要共同标识，不能仅引用 HEAD 冒充已提交实现。具体结果见[环境清单第 2.10 节](../development/environment.md#210-t01-工作区与真实链接验证)及[本月日志](../devlogs/2026-10.md)。
- 日常命令分步输出保留于 `target/t01-*/`；失败日志单独标记。本地检查不代替远端 Actions，不将静态 ELF 当作可启动镜像。

既有远端治理沿用先前证据，本轮未修改或重新读回：master 为默认稳定主线，只经 PR 晋级；允许 merge / rebase，禁用 squash；master Ruleset 25073496 要求严格最新的 Candidate Quality（Actions App 15368），dev 无保护且普通 push 不触发 CI。此前治理 Actions 成功记录见 [master 首跑](https://github.com/laugh0608/RadishRoot/actions/runs/38557500889)与[验证 PR #1](https://github.com/laugh0608/RadishRoot/pull/1)，它们未覆盖本批 Rust 实现。操作流程见[Ruleset 手册](../../.github/rulesets/README.md)。

## 接下来的事项与停止线

1. **先审阅本地 T01 交付。** 必要的本地验证已通过；后续按用户授权处理提交 / push / PR / Actions，不把这些外部动作混入本批。第一次实际运行新增 Linux CI 时记录安装、宿主构建与聚合结果；失败按真实原因修复。
2. **进入 T02 前明确 Limine 配对。** 沿既有 T02 范围核对 bootloader 与 Rust 协议绑定的精确版本、协议修订、资产哈希和许可；实际引入依赖 / 下载资产按明确范围授权。无需重复整套已完成环境盘点。
3. **T02 交付真实启动证据。** 开发镜像、串口、内存图 / framebuffer 必要响应校验、最小 panic 停止；按契约连续 10 次进入内核并验证无效响应拒绝，然后再推进 T03–T05。

**本批停止线：**T01 完成后停止，不开始 T02，不安装 / 更新依赖，不启动 guest，不暂存、提交、推送、创建 PR、启动 Actions 或发布。每周投入预算尚未提供，不承诺后续日历排期。原创许可与实际引入资产的分发条件仍在对应节点核对。

**保留资源：**本批保留忽略的构建缓存、分步日志和独立证据，无新增常驻服务或 VM；测试子进程已回收，专用临时测试目录由测试清理。此前 Rust / QEMU / GPT-FAT 证据与 Docker 工具镜像继续保留，精确身份见环境清单与历史日志，未清理用户或其他任务资源。
