# 开发规范

沿用 Radish 系列的中文协作、明确类型、单维护者串行开发、单一正式来源与按风险验证。工程复杂度以真实问题为依据，不以文件、crate、测试或文档数量衡量完成度。

## 1. 日常推进与工程结构

先核对工作区，再读[当前状态](../status/current.md)和任务对应的[MVP 规划](../mvp-plan.md)。每轮优先完成可以复验的一条行为路径，交接写清启动入口、当前画面 / 能力、证据和下一项任务。

当前 Cargo workspace 仅含 kernel 与 xtask；后续 kernel / platform、system-core / services、shell / ui、host-runner、xtask 按职责分离。开始时可用少量 crate 内模块承载这些边界；出现不同目标依赖、独立测试或稳定复用价值时再拆 crate，不为架构图机械建空目录。

共享核心以 no_std + alloc 为基线；宿主窗口库和 std 后端只在 host-runner / 测试工具中。UI 调用类型化服务，服务通过明确存储边界访问 FAT；文件语义不在不同入口重复实现。

## 2. 代码与资源

- 使用清楚的领域命名、早返回、浅调用层和结构化错误；优先标准库 / 核心库与现有能力，不预建万能 manager、factory、插件或事件总线。
- 沿用现有分层与配置入口，不用字符串标记、隐式全局状态或动态技巧替代明确类型；共享接口 / 格式变化同时复核生产者、消费者、示例、测试及版本 / 兼容语义。
- FileEntry、VolumeInfo、MemoryStats、OperationRecord 等具体类型承担真实职责；公共投影不反过来抹平类型。
- 单文件接近 1000 行时评估职责，原则上不超过 1500 行；按同域职责拆分，不机械切片或增加无意义转发。
- 所有外部输入、地址与长度、整数运算、IO、超时、资源配额和格式边界显式验证。不能用空值或默认成功掩盖损坏。
- unsafe 局限于必要硬件 / 内存边界，注明安全前提、指针来源、物理 / 虚拟映射、对齐、别名、生命周期、并发与释放责任。
- 中断只采集事件；耗时服务在主循环按有界步骤推进。DMA 超时后先确认设备停止访问，不能按普通函数超时释放缓冲区。
- 日常可恢复错误返回 Result；panic 用于无法继续的内部错误并留下串口诊断，不跨 FFI 或回调边界失控展开。
- 新 fallback 必须有触发条件、可观察状态与停止线；不悄悄改写数据、降低持久化或隐藏原始失败。
- 时间用单调时钟表达阶段顺序；跨重启身份包含 boot_id，不能把时钟值当持久唯一 ID。

## 3. 依赖、工具链与版本

引入依赖前说明用途、替代方案、精确版本候选、no_std 特性、上游维护与许可；先实际验证再冻结。Git 依赖固定 commit，提交 Cargo.lock 和 rust-toolchain.toml；不得默认跟随主分支。更新 lockfile、联网下载和修改全局工具链按实际授权执行。

文档基线使用本文所属文档版本；MVP `v0.1` 表示产品能力目标。正式发行沿用家族日历版本 `vYY.M.RELEASE`，开发渠道可附 dev / test；首次工程落地时设计单一产品版本来源，再由构建同步 manifests / 镜像，不在仅文档阶段生成虚假发行标识。协议、磁盘格式、事件 schema 与产品版本分别管理；日历版本变化不替代兼容设计。

固定 QEMU、带版本机器名、OVMF、引导协议和镜像配置；实际支持范围按验证矩阵声明。宿主模拟器不是第二个产品平台。默认不以本机加速模式取代已选 TCG 验收基线。

## 4. 构建入口与检查

构建、镜像、运行、demo、host / VM 测试与安装验证通过同一 xtask 编排；shell 包装不另实现规则。每个入口随真实能力建立，说明参数、资源、时限、输出与错误码，不提供返回成功的空检查。

仓库治理保留独立真实入口 `python3 scripts/check-repo.py`，只依赖 Python 3.10+ 标准库；回归用 `python3 -B -m unittest discover -s scripts/tests -v`。它不运行 Rust 或 VM。未来迁入 xtask 时保留单一实现，不在 Python 与 Rust 中复制两套政策。检查范围及限制见[仓库治理](../governance/repository-governance.md)。

开发工具环境使用 [tools/debian-image/Dockerfile](../../tools/debian-image/Dockerfile) 构建，由容器内的 build-image.sh 核对固定来源并安装工具；脚本不是宿主安装入口，也不是产品制盘 / 运行入口。构建参数、包清单与验收方法统一见[开发环境清单第 2.6 节](environment.md)。后续 xtask 若使用此工具环境，应调用同一配方，不复制包版本与安装逻辑。仓库治理检查只检查这些文件的文本规范，不自动构建镜像或验收工具行为。

| 改动 | 必要相关验证 |
| --- | --- |
| 文档 / 治理 | 本地链接、UTF-8 / LF / 末尾换行、根入口同步、范围 / T / AT 与依赖引用 |
| 领域服务 / 命令 | fmt / check、相关宿主测试、结构化结果与错误、no_std 编译 |
| UI / 输入 | 相同视图宿主复核、真实 QEMU 键盘 / 鼠标 / 焦点 / 状态 |
| 存储 / DMA / 电源 | 目标 guest、边界与失败注入、正常重启、宿主独立读盘 |
| 镜像 / 安装 | 独立副本、危险目标拒绝、读回校验、移除介质与干净 UEFI 启动 |
| 阶段候选 | 对应门槛和 demo；最终候选按全部 AT 及人工清单核对 |

Rust 工程已接入真实 fmt、check、test、clippy 与最终链接检查；只检查文档的工具不能继续被宣称覆盖 Rust。高价值测试验证行为、独立预期、边界和失败，不机械重写实现。低影响文案不要求无关全量测试；只有新变化、失败或未解决风险才扩大复验。

具体命令是否已存在只看实际文件和当前状态。自动化、CI 或脚本未创建时明确未建立，不列出虚假“通过”。文档检查包含未跟踪文件，普通 git diff 无法代表全部文件。

### T01 本地入口

在仓库根使用已安装的项目工具链执行；以下命令不会安装组件或联网解析 crate。nightly 日期由 rust-toolchain.toml 定义，示例对应当前配置；默认工具链与全局环境保持不变。

```sh
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask check
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --host
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask build --profile dev --fresh
env RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask build --profile release --fresh
```

`check` 分别运行宿主 / 裸机 check、clippy（拒绝警告）和 fmt 检查；`test --host` 运行 xtask 宿主测试和 kernel lib 的纯引导、内存、PCI、DMA、块接口、GPT、FAT 字节流与名称、ACPI、时钟 / 等待、队列和计量契约测试（不运行硬件入口）。`build` 实际链接并检查 ELF；这些入口均不执行 guest。`--fresh` 在本次独占目录构建并关闭增量；不带该参数时复用 target/kernel 编译缓存，但仍检查本次返回的产物。尚未实现的命令返回 2。

首次进入 Cargo 的编译发生在 xtask 启动前，应先按上述方式选择工具链；xtask 随后验证自身编译器身份、项目工具链来源与外部配置，不修改 shell、rustup override 或全局默认。当前宿主进程管理使用 Unix 进程组，目标宿主为 macOS / Linux，Windows 不在 T01 支持范围。系统需有 git、shasum 和 /bin/kill；Python 独立复核及 CI 辅助使用标准库，CI 辅助脚本需 Python 3.11+。

每次实际执行在忽略的 target/xtask-<pid>-<时间戳>-<进程序号>/（既有记录无进程序号，早期 T01 记录为 target/t01-*；进程序号防止相同墙钟时间戳碰撞） 保存 steps.txt 与分步 stdout / stderr。xtask 子进程默认上限 120 秒，可加 `--timeout-secs 1..3600`；超时停止自己的进程组并回收直接子进程，返回 124；前置 / 构建 / 校验失败返回 1，参数错误返回 2。错误输出指向日志；不能将失败目录中的半成品当作成功产物。独立验收材料保存在 evidence/，其路径和候选身份记录于环境清单。

CI 的 install 步骤才允许从官方源安装 TOML 指定的工具链，并按 Cargo.lock 执行 `cargo fetch --locked`；本地不运行它。verify 步骤复用上述四个入口；本地验证结果不能替代 Ubuntu runner 的实际 Actions 结果。

### T02 镜像与串口入口

已授权的首次依赖准备使用 `cargo +nightly-2026-10-13 fetch --locked`，Limine 资产准备使用 `python3 scripts/fetch-boot-assets.py --download`；这些操作联网，日常 xtask 不自动下载、安装或更新。资产存在时脚本只校验精确哈希，冲突文件不覆盖。本次 Cargo 缓存隔离在 work/cargo-home；复用本机验收环境时为所有命令显式加 `CARGO_HOME="$PWD/work/cargo-home"`，无需修改全局配置。

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask image
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask run --serial
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm
```

`image` 在新目录构建 release ELF，调用锁定的既有 Debian 工具镜像产生 1 GiB GPT / FAT32 开发盘，输出真实路径。容器禁用网络、只读根、无新增 capabilities，仅挂本次输入 / 输出普通目录，`--pull=never`；工具镜像缺失即失败。工具脚本只由 xtask 编排，参数来自 manifest，不提供第二套独立制盘入口。

`run --serial` 创建镜像并完成一次无窗口 UEFI 启动、只读 GPT / DATA 探测及 T06 循环进展验证，串口保存到本次 boot-*/serial.log。`test --vm` 对正常镜像启动 10 次，再使用独立 boot-test 构建校验 4 个 T02、3 个 T03、7 个 T04、7 个 T05 和 8 个 T06 独立用例；共 39 次启动、30 张引导 / 源盘及 11 张数据副本，逻辑容量均为 1 GiB，实际稀疏占用另记。T04 覆盖写入 / flush、同盘重新启动读回、单 GPT 损坏降级、双损坏拒绝、有效副本冲突拒绝、请求超时隔离及缺少 flush 能力拒绝。`test --vm --storage-only` 复用同一存储用例实现，只运行 7 个 T04 用例，生成 8 张引导 / 源盘与 4 张数据副本；日志明确未执行 T02 / T03 独立回归，不能冒充完整套件。完整套件按资源授权运行，普通 CI 不自动执行 VM。

每次复制全新 VARS；固定 TCG、pc-q35-11.1、qemu64、1 vCPU / 512 MiB、VGA、modern VirtIO-blk，无网络、无 GUI、禁止自动重启。普通盘只读；T04 / T05 双盘用例由固件从只读 virtio-scsi / scsi-hd 引导，内核仅操作唯一 VirtIO-blk 数据盘。t04-write 及 T05 写入 / 同步失败用例打开各自新建数据副本的写权限，使用 cache=writeback、aio=threads、无 snapshot；T06 timeout 使用独立只读 DATA 副本，其他数据盘只读。所有盘均为任务普通文件，不挂宿主物理盘。固件路径从 QEMU 实体 prefix/share/qemu 派生并核对哈希；实际 VM 环境是 Homebrew macOS，其他安装布局未验证。

单次 QEMU 限时为 `min(--timeout-secs, 60)` 秒。宿主按模式观察终态且串口稳定 250 ms 后停止自己的进程组并回收，再严格校验序列。正常 / T06 等待 T06 READY / FAIL 或更早阶段的失败；T04 READY 不能提前结束正常启动。T02–T05 独立用例保留原有终态，T02 / T03 READY 只是中间阶段。提前退出、无终态、非预期错误或事实不符均失败；超时返回 124。这是宿主终止：正常 T06 READY 后仍继续循环，预期致命错误则 cli / hlt。重新打开同盘验证不代表正常重启 / 关机验收。没有 UART 或 CPU 无进展时由宿主超时报告。

正常请求需真实 used token 与响应状态成功，flush 单独确认；停止时确认设备 status=0、关闭 bus master 后释放 queue。请求以 HPET 经过时间限制为 2 秒、复位为 100 毫秒，另保留请求 2000000 次、复位 100000 次独立保险；先达到次数上限时报 PollBudgetExhausted，时钟故障另报 Clock，不混称真实超时。配置一致性仍限 64 次。超时用例实际发布请求再隐藏完成观察，保留全部 DMA 与原始缓冲、拒绝释放 / 复用后 halt；不声称制造了设备物理延迟。缺 flush 用例是明确标记的能力拒绝注入；不声称 QEMU 实际未提供该能力。

写入用例在 DATA 尾部 4096 字节合成区域执行对齐与跨扇区写入并 flush；guest 退出后宿主按独立期望比较整盘，确认 ESP / GPT 及所有非目标字节不变，然后重新启动同一盘只读复核。各次构建、工具身份、命令、PID、镜像摘要、串口、VARS 与失败材料留在 target/xtask-*；后台子进程及容器结束即回收，复制证据不复制大盘。T04 不提供 FAT 文件语义、正常电源流程、GUI 或安装。

### T05 FAT 持久化与独立读取入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --fat-only
```

`--fat-only` 复用完整 suite 中同一 T05 编排，生成 8 张引导 / 源盘与 6 张 DATA 副本，执行 7 次启动：写入、同盘只读重开、只读拒绝、文件目录项写回失败、卷 FSInfo 写回失败、最终设备 flush 失败、损坏 FAT 拒绝。单次仍为 60 秒上限；不得与 `--storage-only` / `--runtime-only` 同时使用，分组成功不代表整套 `test --vm` 通过。默认 `run --serial` 完成 T04 只读探测后进入 T06 循环，FAT 写入仅由 boot-test 白名单调用；T08 的正式文件服务尚未建立。

DATA 挂载只经 GPT 验证的 Partition；字节流每次最多操作一个扇区内的字节，不增加缓存。首次 IO 错误锁存并拒绝后续设备访问，避免 File / FileSystem 的 Drop 自动重试隐藏错误。每次挂载最多 100000 次流操作（含 seek），用完明确失败，这是防止异常遍历的次数边界，不是实时超时或未来文件服务的长期配额。成功顺序为文件显式 flush、释放目录句柄、卷 flush、显式 unmount、最终设备 flush；只读挂载无脏写入，流 flush 是无写入动作且不会发设备请求。具体名称限制见 [T05 实施契约](../mvp-plan.md#t05-本批实施契约)。

预置材料由 mtools 写入本次副本；guest 写入后，宿主先结束并 wait QEMU，再通过 `tools/boot/check-fat.sh` 调用固定 mtools / dosfstools。磁盘以只读 bind 挂入无网络工具容器，`mdir -b -s` 核对精确名称，`mcopy` 提取到 ASCII 案例文件名，避免宿主路径规范化混淆 `é` 与 `e` + combining acute；比较长度、字节、SHA-256，分区副本仅运行 `fsck.fat -n`。原磁盘不挂载写入、不修复；DATA 外字节与源盘逐块比较。工具容器限制 1 CPU / 256 MiB / 64 PID，结束按唯一名称回收。

文件 / 卷写回失败是流入口的显式单次写失败注入，后续尝试由首错锁存拒绝；最终 flush 失败将该次 VirtIO 请求类型改为 unsupported，核对真实设备负响应及 Flush 阶段。两者不冒充物理设备损坏。失败副本仍独立观察，记录零长文件、未引用簇、脏标记或过期 FSInfo；文件可读不能推翻先前失败，也不能宣称持久化已确认。无正常关机、重启命令或断电事务保证。

### T06 异常、时钟与循环入口

```sh
env CARGO_HOME="$PWD/work/cargo-home" RUSTUP_AUTO_INSTALL=0 cargo +nightly-2026-10-13 xtask test --vm --runtime-only
```

实施与失败契约统一见 [T06 实施契约与验证方案](../mvp-plan.md#t06-实施契约与验证方案)。`--runtime-only` 与其他分组互斥，复用全套同一用例函数，执行 1 次正常循环和 8 个独立用例，生成 9 张引导 / 源盘与 1 张只读 timeout DATA 副本。分组不包含 T02–T05 独立回归；本轮定向尝试首例失败后已停止，九类 T06 的通过证据来自最终 39 次完整套件，不能声称定向组另行完整通过。

固定平台使用经 ACPI 表与共享 MMIO 映射校验的 64 位 HPET，period 实测 10000000 fs；PIC / PIT IRQ0 仅唤醒主循环，显式关闭并读回本地 APIC enable 位，不支持 x2APIC。IRQ 只更新原子 tick / pending 并 EOI，不分配、打印或调用文件服务。64 项 FIFO 满时拒绝新事件并计数，主循环每轮最多处理 16 项；内存统计分别报告 Usable 帧与堆范围，未知部分不计为空闲，堆不能重复加入帧分配总数。

早期 IDT 保护启动路径；最终 GDT / TSS / IDT 为 double fault 配置静态 16 KiB IST 和 4 KiB guard。致命异常、panic 与分配失败使用最多 2048 字节、每字节最多 100000 次发送轮询的紧急串口，禁止堆分配、普通锁与析构展开。page fault / double fault / OOM 通过真实机制注入；普通非法名称拒绝后仍处理合法事件。时钟停止、缺失、倒退与不一致不静默 fallback；已提交请求失败原地隔离 DMA，不退出被设备引用的栈帧。

实际结果、首次 IRQ 失败修复、独立 ELF / 磁盘 / 坏日志对照及资源统计见[环境清单第 2.17 节](environment.md#217-t06-异常时钟与事件循环验收)。仅启用已批准的 x86_64 0.15.5 中断 ABI 特性，无新增包、下载、工具链或 lockfile 变化。后续新依赖或新增资源仍按任务授权，不把本轮额度延伸到 T07。

### T07 入口规划（尚未实现）

T07 的显示 / 输入所有权、PS/2 状态机、队列丢失恢复、同视图宿主 runner、依赖及资源方案统一见 [T07 实施契约与验证方案](../mvp-plan.md#t07-实施契约与验证方案)。T07 只交付基础输入 / 显示校验页；T10 的真实双面板和 AT07 完整控件验收仍待后续实现。

拟新增的 `ui --host`、`ui --host --gui`、`run --gui`、`test --vm --graphics-only` 当前均不存在，不能作为已可执行命令。计划由 xtask 编排独立 host-runner 和既有 QEMU，宿主默认无窗口 / 无 SDL，GUI 显式启用；QMP 使用任务私有 Unix socket，真实 PS/2 注入、状态断言及实际 screendump 联合判定，不能只以 READY 文本或共享渲染器自证正确。

实施所需依赖、锁变更及 VM / GUI 资源以[当前状态](../status/current.md#已确认范围与停止线)记录的用户授权为准，不因入口尚未实现而重复请求已覆盖的授权。实际引入时核对精确依赖闭包、许可与平台兼容性，按预算运行并保留失败证据；方案确认不等于下载、构建或验收已执行。原有 T06 入口与通过证据保持不变。

## 5. 视觉与数据材料

采用 RadishX family-ui v26.7.3 的 Workbench 方向，项目映射以 MVP 规划为准。参考规则而非搬运外部截图、品牌或整包 UI；具体设计资产若使用 .pen，只通过支持的 Pencil 工具读写。

UI 状态随着真实行为扩展，优先可读性、焦点、错误、长内容与资源上限。产品语言与文档语言分别处理，英文界面不允许损坏 UTF-8 路径。

颜色、间距和状态样式优先由已有语义 token 或明确的项目映射表达，不让不同页面随意增加硬编码值。复用组件与服务边界，分别核对视觉可读性及真实键盘 / 指针 / 滚动行为；不把设计图当作已实现的交互证据。

测试只用合成内容与任务专用磁盘；大镜像、敏感日志和缓存不入 Git。生成物记录生成入口、输入与版本；关键数据使用独立工具或期望复核。

## 6. 文档与交付

README 稳定定位；AGENTS / CLAUDE 启动级规则；产品 / MVP 专题维护正式契约；current 只记当前能力、阻塞、下一步；月度 devlog 保存过程；ADR 保存关键决定。具体职责见[文档导航](../README.md)。

提交遵循[仓库治理](../governance/repository-governance.md)，授权与清理遵循[协作规则](../governance/agent-collaboration.md)。交付写最终结果、实际证据、未验证项、风险和仓库状态；不把静态检查或宿主演示扩大成产品完成。
