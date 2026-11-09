# 当前状态

更新日期：2026-11-09（Asia/Shanghai）。

## 规划阅读链

[产品整体规划](../product-plan.md) → [MVP 五个大专题](../mvp-plan.md#capability-map) → [安装与系统生命周期](../mvp-plan.md#installation-lifecycle) → [T13 M1 验收入口](../mvp-plan.md#t13-m1-演示入口与首轮真实验收)。任务状态、FR / AT 与完成规则以 MVP 原任务为准。

## 当前结论

**T13 进行中。首轮 `xtask demo` 的真实 QEMU 自动主路径、独立读盘和隔离同步失败对照通过。** 详细证据见[环境清单第 2.25 节](../development/environment.md#225-t13-m1-自动演示首轮验收)。

| 层级 | 实际状态 |
| --- | --- |
| Git 与 CI | [PR #3](https://github.com/laugh0608/RadishRoot/pull/3) 的 Candidate Quality 通过，已以 `c55ff01` 合入 master 并回流。origin/master 与 origin/dev 均指向该提交，祖先关系已复核。T13 首轮实现与验收记录随本次提交保存在本地 dev。 |
| 入口 | [`xtask demo`](../development/standards.md#t13-m1-自动演示入口) 运行普通 release 内核。首启创建 `hello.txt` 后自动刷新，详情与写入同 ID，同名被拒。两次 QMP `guest-shutdown` 均自然退出为 0。宿主独立读回 16 字节，次启只读打开同一 DATA，内容一致，旧操作返回 `ForeignBoot`，整盘摘要一致。 |
| 故障对照 | `t12-volume-fail` 用独立 DATA 注入卷同步失败，界面保留原阶段与原 ID，后续写入被拒，诊断仍可执行。宿主回收该进程，独立读盘通过。 |
| 本地验证 | fmt、宿主与裸机 check / 严格 Clippy、112 项宿主测试（30 xtask、82 kernel）、dev / release 静态 ELF 链接通过。验证目录在 `target/xtask-78823-1794222268704602000-0/`、`target/xtask-83706-1794222284153824000-0/`、`target/xtask-84377-1794222296055526000-0/` 和 `target/xtask-84916-1794222303212292000-0/`。 |
| 未验证面 | 人工 GUI、Cocoa 实体修饰键、AT06 宿主与 guest 等价复验、AT07 交互与 M1 资源测量仍待执行。AT05 记录自动部分覆盖，M1 保持进行中。 |

## 接下来的事项

1. 补人工 GUI 键鼠与焦点、Cocoa 实体修饰键、AT06 等价、AT07 及 M1 输入延迟和内存测量。完整套件和阶段候选复验按影响与资源另列。
2. M1 退出按 [T13 通过条件](../mvp-plan.md#t13-m1-演示入口与首轮真实验收) 收口，先解决 P0 / P1。目录、预览、复制与安装沿用原阶段。

## 已确认范围与停止线

- 首轮批准最多 6 次无窗口 VM、10 张专用稀疏 raw，单次最多 60 秒，盘及 FAT 分配 512 MiB、其他产物 2 GiB，总执行 15 分钟。实际 3 次、5 张 raw、9 份 FAT，盘及 FAT 约 47.5 MiB，其他约 198.6 MiB，运行窗口 74 秒。进程、容器与私有 socket 已核对回收，保留盘、日志与帧。后续运行另定预算。
- T12 专用预算、既有盘、VARS、日志及资源退出记录见[环境清单第 2.24 节](../development/environment.md#224-t12-正常同步与电源验收)。原创许可与每周投入仍待所有者决定。
