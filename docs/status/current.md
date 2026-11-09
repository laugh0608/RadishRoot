# 当前状态

更新日期：2026-11-09（Asia/Shanghai）。

## 规划阅读链

[产品整体规划](../product-plan.md) → [MVP 五个大专题](../mvp-plan.md#capability-map) → [安装与系统生命周期](../mvp-plan.md#installation-lifecycle) → [T13 M1 验收入口](../mvp-plan.md#t13-m1-演示入口与首轮真实验收)。任务状态、FR / AT 与完成规则以 MVP 原任务为准。

## 当前结论

**T13 进行中。自动主路径通过，Cocoa 首窗部分覆盖。** 人工入口已验证实体 Shift 引号、自动列表和鼠标打开同 ID 详情，首窗达到时限后回收。证据见[环境清单第 2.25 节](../development/environment.md#225-t13-m1-自动演示首轮验收)及[第 2.26 节](../development/environment.md#226-t13-cocoa-人工演示首窗部分证据)。

| 层级 | 实际状态 |
| --- | --- |
| Git 与 CI | [PR #3](https://github.com/laugh0608/RadishRoot/pull/3) 的 Candidate Quality 通过，已以 `c55ff01` 合入 master 并回流。origin/master 与 origin/dev 均指向该提交，祖先关系已复核。T13 自动演示实现与首轮验收记录已在本地 dev 提交为 `408bac8`。 |
| 入口 | [`xtask demo`](../development/standards.md#t13-m1-自动演示入口) 运行普通 release 内核。首启创建 `hello.txt` 后自动刷新，详情与写入同 ID，同名被拒。两次 QMP `guest-shutdown` 均自然退出为 0。宿主独立读回 16 字节，次启只读打开同一 DATA，内容一致，旧操作返回 `ForeignBoot`，整盘摘要一致。 |
| 故障对照 | `t12-volume-fail` 用独立 DATA 注入卷同步失败，界面保留原阶段与原 ID，后续写入被拒，诊断仍可执行。宿主回收该进程，独立读盘通过。 |
| 人工入口 | [`xtask demo --gui`](../development/standards.md#t13-cocoa-人工演示入口与清单) 沿用同盘生命周期，等待操作者键鼠输入并取帧。每窗时限取 `--timeout-secs` 与 600 秒的较小值。人工入口及首窗部分证据随本次提交保存在本地 dev。 |
| 本地验证 | 人工入口候选的 fmt、宿主与裸机 check / 严格 Clippy、114 项宿主测试（32 xtask、82 kernel）、dev / release 静态 ELF 链接通过。目录为 `target/xtask-10374-1794223676994630000-0/`、`target/xtask-11313-1794223690250033000-0/`、`target/xtask-11886-1794223701119161000-0/` 和 `target/xtask-12407-1794223707671684000-0/`。 |
| 未验证面 | Cocoa 的 Tab / Enter、冲突详情、正常关机、独立读盘、同盘次启和实体 Shift 冒号待补。AT06 宿主与 guest 等价、完整 AT07 与 M1 资源测量仍待执行。AT05 记录部分覆盖。 |

## 接下来的事项

1. 单独确认新一轮窗口额度后，用新专用 DATA 从首窗重做 [Cocoa 人工清单](../development/standards.md#t13-cocoa-人工演示入口与清单)。窗口就绪时先确认已置前，再交接实体键鼠。按两次同盘启动补齐人工闭环证据。
2. 接续 AT06 等价、AT07 及 M1 输入延迟和内存测量。完整套件和阶段候选复验按影响与资源另列。M1 退出按 [T13 通过条件](../mvp-plan.md#t13-m1-演示入口与首轮真实验收) 收口，先解决 P0 / P1。

## 已确认范围与停止线

- 首轮批准最多 6 次无窗口 VM、10 张专用稀疏 raw，单次最多 60 秒，盘及 FAT 分配 512 MiB、其他产物 2 GiB，总执行 15 分钟。实际 3 次、5 张 raw、9 份 FAT，盘及 FAT 约 47.5 MiB，其他约 198.6 MiB，运行窗口 74 秒。进程、容器与私有 socket 已核对回收，保留盘、日志与帧。后续运行另定预算。
- Cocoa 一轮额度已结束。批准两次顺序启动、每窗 300 秒，2 张专用稀疏 raw，盘及 FAT 分配 256 MiB、其他产物 1 GiB，总执行 15 分钟。实际首窗超时返回 124，使用 2 张 raw、3 份 FAT，盘及 FAT 约 18.5 MiB，其他约 112.5 MiB，运行窗口 314 秒。进程、容器和私有 socket 已核对回收，保留失败盘与证据。
- T12 专用预算、既有盘、VARS、日志及资源退出记录见[环境清单第 2.24 节](../development/environment.md#224-t12-正常同步与电源验收)。原创许可与每周投入仍待所有者决定。
