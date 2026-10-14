# 开发环境清单与安装候选

盘点日期：2026-10-14（Asia/Shanghai）；对应 MVP 文档 2.0 的 T01。本文维护环境事实、精确候选和验证方法；任务进度、下一步和授权停止线只看[当前状态](../status/current.md)。本机盘点与上游元数据核对不代表安装、编译或运行成功。

## 1. 本机实测

检查范围为 PATH、Rust 已安装工具链、Homebrew Cellar、UTM / OrbStack 应用包、现有 Docker 服务及本仓库。未全盘搜索，未读取个人磁盘或进入现有容器；“未发现”仅指上述范围。工具路径用 `~` 表示当前用户目录，不能将本机绝对路径写死到项目构建配置。

| 项目 | 版本 / 路径与证据 | 适用性与缺口 |
| --- | --- | --- |
| 宿主 | Darwin arm64；macOS 27.0.1，build 26A434 | x86_64 guest 按规划使用 TCG；未测 CPU / 内存性能 |
| Rust 1 | `1.96.0-aarch64-apple-darwin`；rustc `ac68faa20`，Cargo 1.96.0，LLVM 22.1.2 | 有 rustfmt、Clippy、rust-src；已装宿主、iOS、iOS simulator、Android ARM64 target，缺 `x86_64-unknown-none` |
| Rust 2 | `1.97.1-aarch64-apple-darwin`；rustc `8bab26f4f`，Cargo 1.97.1，LLVM 22.1.6 | 有 rustfmt、Clippy；只有宿主 target，缺 rust-src 和裸机 target |
| 工具链选择 | `rustup show active-toolchain` 返回 1.96.0，由 `RUSTUP_TOOLCHAIN` 环境变量覆盖 | 两套均为稳定版，未装 nightly；后续项目命令须显式选择候选，不能只依赖 rust-toolchain.toml |
| ELF 链接器 | 两套工具链的 `lib/rustlib/aarch64-apple-darwin/bin/rust-lld` 分别报告 LLD 22.1.2 / 22.1.6 | 已有 ELF 链接器；尚未链接本项目内核，无须预先增加完整 LLVM 安装 |
| 本机 C 工具链 | Apple clang 21.0.0，默认 target `arm64-apple-darwin27.0.0` | 默认宿主配置不能视为 x86_64 裸机 C 工具链 |
| 独立 QEMU CLI | PATH 未发现 `qemu-system-x86_64`、`qemu-img`；Homebrew Cellar 无 qemu | 项目自动化运行入口缺失 |
| UTM | `/Applications/UTM.app` 4.7.5；内置 QEMU 与 qemu-img 动态库含 10.0.2 版本字符串 | `file` 确认是 Mach-O shared library，并非可直接运行的 CLI；版本来自静态字符串，未执行 guest |
| UTM 固件 | `Contents/Resources/qemu/edk2-x86_64-code.fd` 与 `edk2-i386-vars.fd` | 配对 JSON 声明 x86_64、pc-q35-*；没有启动证据，不能把 ARM 固件代入 |
| Linux 服务 | OrbStack 2.2.4；Docker context 为 orbstack，CLI / server 均为 29.8.2，服务为 linux / aarch64 | 只读 API 已成功；没有启动、拉取或进入容器，制镜像包状态未知 |
| 镜像 / 引导工具 | PATH 未发现 sgdisk、gdisk、mkfs.fat、fsck.fat、mformat、mcopy、mdir、xorriso、limine | Linux 工具环境与 Limine 资产尚未建立；不是全机不存在的断言 |
| 系统辅助工具 | hdiutil、diskutil、newfs_msdos、fsck_msdos、file、shasum 存在 | 不用这些存在性证据替代规划中的固定 Linux GPT/FAT 工具环境 |

Rust 可执行文件位于 `~/.rustup/toolchains/<toolchain>/bin/`；本轮直接调用已安装二进制及 rustup 的 installed 查询，未执行工具链安装或更新。官方说明 `x86_64-unknown-none` 有预编译组件，支持 no_std，使用 alloc 需提供 allocator；目标本身不强制 nightly。项目仍沿用固定日期 nightly 方向，具体依赖功能通过实际编译确认。[Rust target 说明](https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html)

UTM 的固件 JSON 保留上游构建机路径，不能原样复用。以下 SHA-256 是本机文件实测，仅用于识别此次盘点样本，不代表项目选择了这些固件：

| UTM 文件 | 字节数 | SHA-256 |
| --- | --- | --- |
| edk2-x86_64-code.fd | 3653632 | `d23e35c96cbf47468e5c7c2ce82cfa0c825d4e916bb6bacbf3d8d81556350fe3` |
| edk2-i386-vars.fd | 328704 | `a5c722949ba13c60b08c1ad3507ef751424941781f66d529f5f1feded8320ccb` |

## 2. 精确安装候选

以下是拟验证的版本组合，不是已冻结的产品依赖。版本号、包修订和架构一并核对；安装时如果来源内容变化、指定版本不可用或校验不符，应报告差异并调整方案，不能自动切换 latest。元数据均于 2026-10-14 核对。

| 层次 | 精确候选 | 来源与核对结果 |
| --- | --- | --- |
| Rust | `nightly-2026-10-13-aarch64-apple-darwin`，rustc `1.101.0-nightly (e9bbadf68 2026-10-12)` | 官方日期 manifest 已读取；宿主工具链、`rust-std/x86_64-unknown-none`、rust-src、rustfmt-preview、clippy-preview 均为 available=true |
| Rust 安装集 | minimal profile + rust-src、rustfmt、clippy；target 为 `x86_64-unknown-none` | 不增加其他 target；使用工具链自带 rust-lld，暂不增加 llvm-tools、GDB 或交叉 GCC |
| QEMU | Homebrew `qemu 11.1.2`，formula revision 0、bottle rebuild 0，`arm64_golden_gate` | 官方 formula API 提供匹配 macOS 27 ARM64 的 bottle；来源身份及哈希见下表 |
| x86_64 UEFI | 上述 QEMU 11.1.2 包内的 `edk2-x86_64-code.fd` + `edk2-i386-vars.fd` | QEMU 对应 tag 的 README 标明固件源为 `edk2-stable202302`；最终以安装包文件、descriptor 与实测哈希核对，不借用 UTM 文件 |
| Linux 基础环境 | `debian:trixie-20261005-slim`，`linux/arm64/v8` | Docker Official Images 声明该日期标签；构建产物索引的 ARM64 manifest digest 见下表，registry 对应关系仍需读回确认 |
| GPT | Debian trixie ARM64 `gdisk=1.0.10-2+b1` | 提供 sgdisk；不能误用该包 amd64 的 `1.0.10-2` 修订 |
| FAT 格式 / 检查 | Debian trixie ARM64 `dosfstools=4.2-1.2` | 提供 mkfs.fat、fsck.fat |
| FAT 文件访问 | Debian trixie ARM64 `mtools=4.0.48-1` | 提供 mcopy、mdir 等，可操作普通镜像文件 |

Rust manifest：[固定日期清单](https://static.rust-lang.org/dist/2026-10-13/channel-rust-nightly.toml)。网页工具未能读取该资源，已通过只读 HTTPS 文本请求成功解析；没有下载工具链归档。

QEMU：[formula API](https://formulae.brew.sh/api/formula/qemu.json)、[formula 源码](https://github.com/Homebrew/homebrew-core/blob/master/Formula/q/qemu.rb)、[固定 release 的固件来源](https://github.com/qemu/qemu/blob/v11.1.2/pc-bios/README)、[x86_64 配对 descriptor](https://github.com/qemu/qemu/blob/v11.1.2/pc-bios/descriptors/60-edk2-x86_64.json)。仅用包版本不能推断固件已兼容；安装后还需记录解包文件哈希及实际启动结果。

Linux：[Official Images 标签声明](https://github.com/docker-library/official-images/blob/master/library/debian)、[ARM64 构建产物索引](https://github.com/debuerreotype/docker-debian-artifacts/blob/dist-arm64v8/trixie/slim/oci/index.json)、[gdisk](https://packages.debian.org/trixie/gdisk)、[dosfstools](https://packages.debian.org/trixie/dosfstools)、[mtools](https://packages.debian.org/trixie/mtools)。基础镜像标签不锁定后续 apt 软件源；正式工具镜像须同时记录软件源快照、完整 dpkg 清单和最终镜像 digest，不能仅靠这三项顶层版本宣称传递依赖已经锁定。

| 对象 | 元数据提供的 SHA-256 / digest |
| --- | --- |
| Rust 宿主完整发行归档（供识别，不表示 minimal 会下载整个归档） | `b3344b44dd3af7a6864c1156a07b40610cda3b6eaa9bbecb89cb7f91e9c3e2d3` |
| Rust 裸机 target 的 xz 归档 | `316173a4802efa9e6a87e37e9678019a66ef5da7b1e51d85b0e83f8bd74583d0` |
| QEMU 11.1.2 source tar.xz | `731b5681e4bb18be313231579b8efd0296c5b015fa36dc533874b639ba838016` |
| QEMU arm64_golden_gate bottle | `a399faaf2f8fb45d3b1f35704041a9104f7f717e9fda1b12d4deac9e919d0d33` |
| Debian ARM64 构建产物 manifest | `sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850` |

这些归档未下载，哈希来自官方文本元数据；Debian digest 目前是构建产物索引证据，尚未证明 Docker Hub 的日期标签解析到它。实施前用 registry manifest 查询确认日期标签、平台和 digest 一致，再以 digest 拉取；不拉取 layers 来完成这一步核对。

QEMU formula API 此次报告 tap commit 为 `6aa7c839d4bda90f76fba6805e9a96bdd0dd21cf`，formula SHA-256 为 `b8fa843b66a8879edf2f4c2440a4990557762764fc7f46767a26929d355215c7`。该 commit 的源码 URL 本次未读到，不能声称已核验提交内容。安装前再次对照上述版本 / bottle 身份；普通 `brew install qemu` 会随 formula 变化，不能视为永久精确安装命令。

## 3. 安装影响与清理边界

| 动作 | 写入与资源影响 | 恢复 / 清理方式 |
| --- | --- | --- |
| 安装指定 nightly 和组件 | 写入 `~/.rustup/toolchains/` 与下载缓存；可能占用数 GiB，实际以下载清单为准 | 保留现有 1.96.0 / 1.97.1 与默认设置；仅在确认未被其他项目使用时移除本任务新增工具链 |
| 安装 QEMU bottle | 写入 `/opt/homebrew/Cellar`、链接目录和 Homebrew 缓存；会引入传递依赖 | 安装前后记录差异；只处理本次新增且无其他使用者的项，不做全局 autoremove / cleanup |
| 构建 Linux 工具镜像 | OrbStack 镜像存储、构建缓存、包索引及专用输出目录产生写入 | 记录项目镜像 ID / digest 和容器 ID；退出本任务容器，仅清理归属明确的资源，不执行全局 prune |
| 后续制盘 / VM 验证 | 每份 raw 逻辑容量 1 GiB，副本数与实际占用另记；每次运行有独立 VARS 和日志；guest 基线 1 vCPU / 512 MiB，宿主另有仿真开销 | 仅使用仓库忽略目录内的任务普通文件，不挂宿主物理盘；结束关闭任务进程，保留失败副本时记录路径 |

QEMU formula 的直接运行依赖为 capstone、dtc、glib、gnutls、jpeg-turbo、libpng、libslirp、libssh、libusb、lzo、ncurses、pixman、snappy、vde、zstd。本机已见 snappy 1.2.2，但是否满足当时解析结果仍须核对；不能据此断言完全没有升级现有包的影响。优先匹配 bottle；若退化为源码构建，会引入额外构建工具并显著延长时间，应先重新说明范围。

时间仅作预算：Rust 与 QEMU 下载安装合计约 10–30 分钟，首次 Linux 工具镜像构建约 5–15 分钟，受网络与缓存影响，尚未测量。安装前列出包管理器实际解析的新增 / 升级项、总下载量与磁盘需求；不运行全局 upgrade，不改变 shell 配置或现有 Docker 默认 context。

Limine 二进制与 Rust crate 的配对、业务 crates / 功能开关、SDL 宿主 runner 依赖属于后续实现所需的单独候选，不包含在以上宿主工具安装集内。T01 最小 no_std 编译不需要先安装所有 T02–T05 依赖；形成引导镜像前必须完成 Limine 协议修订、资产哈希与许可核对。当前 raw / UEFI 方向不预装 xorriso、NASM、完整 LLVM 或 Linux Live 介质，确有实现需要时再补具体版本。

第三方项尚未引入仓库。Rust 工具链、QEMU、EDK II、Debian 基础系统与每个包分别按[许可与来源策略](../governance/licensing-and-sources.md)登记；QEMU formula 的 GPL-2.0-only 不覆盖所有附带固件，EDK II 另核对 `edk2-licenses.txt`。本表不是分发许可结论，也不承诺打包这些宿主工具。

## 4. 分层验证方案

以下均为后续执行方案，不能勾作本轮通过。安装完成与业务实现完成分开验收。

| 层次 | 方法与通过条件 | 失败处理 / 证据边界 |
| --- | --- | --- |
| 安装前身份核对 | Rust 日期 manifest 与组件可用；QEMU 版本、revision、bottle 哈希匹配；Debian registry manifest 平台及 digest 匹配；软件源含指定 ARM64 包修订 | 缺包、哈希不符、版本漂移则停在该项，不自动换源、升级或选 latest；列出传递依赖差异再执行 |
| Rust 工具可运行 | 显式 `cargo +nightly-2026-10-13 -V` / `rustc +nightly-2026-10-13 -Vv`，核对已装 target / 组件及 rust-lld | 避免 `RUSTUP_TOOLCHAIN=1.96.0` 干扰；不改用户全局默认；版本输出仅证明工具可执行 |
| QEMU 功能枚举 | `qemu-system-x86_64 --version`、`-machine help`、`-accel help`、`-display help` 与 `qemu-img --version`；记录实际支持的精确 pc-q35-* 名称、TCG 和显示后端 | 当前没有独立 CLI，因此不编造最终机器版本；若缺 TCG 或所需后端，不降低为另一架构 / 加速器冒充通过 |
| 固件核对 | 读安装包 descriptor、许可证、两个文件尺寸与 SHA-256；代码只读，VARS 每次从模板复制 | 描述文件与实体路径不一致、架构不符则失败；不能用“有 .fd”证明可启动 |
| Linux 工具可运行 | 在项目专用 ARM64 容器读取 dpkg 精确包版本、sgdisk / mkfs.fat / mtools 版本；记录基础与最终镜像 digest、完整包清单 | 不进入其他项目容器；没有目标包不得报告环境就绪；不使用 privileged 或透传宿主设备 |
| 工具制镜像验证 | 只对新建普通文件生成规划中的 1 GiB / 512 字节扇区 GPT、128 MiB FAT32 ESP 和 DATA；检查主备 GPT、边界及 FAT；合成文件复制后用独立读取路径对照内容哈希 | 不把制盘工具成功当作内核持久化通过；读盘前确保无活动 guest，不自动修复损坏样本 |
| T01 工程验证 | 实现获授权后建立真实 Cargo workspace 和入口；干净输出目录编译 x86_64-unknown-none 最小 no_std 内核，确认 ELF x86_64；宿主与内核依赖图隔离，记录实际 features / lockfile | 不能用宿主 cargo check 或版本输出替代裸机编译；无入口时不建占位成功检查 |
| T02–T05 / M0 | 固定精确机器名、CPU / 设备参数、TCG、固件及镜像哈希；取得引导、写入、flush、重启读回、宿主独立校验与失败证据 | 依 MVP 的 T / AT 执行；UEFI 启动画面、最小编译或模拟器通过均不能替代 M0 完成 |

ARM64 宿主模拟 x86_64 使用 TCG，硬件虚拟化要求宿主与 guest 架构匹配；不以 ARM64 容器可运行推断 x86_64 guest 已验证。[UTM 架构说明](https://docs.getutm.app/settings-qemu/system/)、[QEMU 加速器与能力枚举](https://www.qemu.org/docs/master/system/introduction.html)

## 5. 盘点限制与复核入口

本机事实来自 `uname` / `sw_vers`、`command -v`、rustup installed 查询、已有 rustc / cargo / rust-lld 的版本输出、应用 Info.plist、`file` / `strings`、固件 descriptor / `shasum` / 文件尺寸，以及 Docker 格式化只读 API。UTM 动态库字符串不是独立 QEMU 的运行结果；本机性能、夜间工具链实际编译、registry 日期标签映射及所有产品行为均未验证。

Homebrew 的 `list --versions` 即使设置 NO_AUTO_UPDATE 仍尝试刷新 API，访问代理失败；其失败不能算安装清单查询成功，后续改为只读 Cellar。Docker API 首次被沙盒拒绝，经同一只读目标的权限批准后返回服务版本与架构。sysctl 硬件查询被权限限制，未据此填入 CPU 型号或内存容量。以上都没有启动服务或安装依赖。
