# 开发环境清单与安装候选

盘点日期：2026-10-14；工具文件操作、正常 Homebrew 方案及安装后核验：2026-10-19；T01–T06 验证与 M0 收口：2026-10-25（Asia/Shanghai）；对应 MVP 文档 2.0。本文维护环境事实、精确候选和验证方法；任务进度、下一步和授权停止线只看[当前状态](../status/current.md)。本机盘点与上游元数据核对不代表安装、编译或运行成功；QEMU 与指定 Rust nightly 均已由用户安装，实际核验范围见第 2.8–2.17 节。

## 1. 本机实测

检查范围为 PATH、Rust 已安装工具链、Homebrew Cellar、UTM / OrbStack 应用包、现有 Docker 服务及本仓库。未全盘搜索，未读取个人磁盘或进入现有容器；“未发现”仅指上述范围。工具路径用 `~` 表示当前用户目录，不能将本机绝对路径写死到项目构建配置。

| 项目 | 版本 / 路径与证据 | 适用性与缺口 |
| --- | --- | --- |
| 宿主 | Darwin arm64；macOS 27.0.1，build 26A434 | x86_64 guest 按规划使用 TCG；未测 CPU / 内存性能 |
| Rust 1 | `1.96.0-aarch64-apple-darwin`；rustc `ac68faa20`，Cargo 1.96.0，LLVM 22.1.2 | 有 rustfmt、Clippy、rust-src；已装宿主、iOS、iOS simulator、Android ARM64 target，缺 `x86_64-unknown-none` |
| Rust 2 | `1.97.1-aarch64-apple-darwin`；rustc `8bab26f4f`，Cargo 1.97.1，LLVM 22.1.6 | 有 rustfmt、Clippy；只有宿主 target，缺 rust-src 和裸机 target |
| Rust nightly | `nightly-2026-10-13-aarch64-apple-darwin`；rustc / Cargo 1.101.0-nightly，LLVM 23.1.3 | 七项所需组件和裸机 target 已安装，安装后核验见第 2.9 节；T01 本地编译与链接已通过，见第 2.10 节 |
| 工具链选择 | 默认和 `RUSTUP_TOOLCHAIN` 仍选择 1.96.0；显式 `+nightly-2026-10-13` 查询正确进入 nightly | 三套工具链并存；项目命令及子进程必须选择指定 nightly，不能只依赖 rust-toolchain.toml |
| ELF 链接器 | 两套稳定版的 rust-lld 为 LLD 22.1.2 / 22.1.6；新 nightly 自带 LLD 23.1.3，GNU 模式版本查询通过 | nightly 链接器加载同一工具链的 libLLVM；T01 两种 profile 真实链接已通过，无须增加完整 LLVM 安装 |
| 本机 C 工具链 | Apple clang 21.0.0，默认 target `arm64-apple-darwin27.0.0` | 默认宿主配置不能视为 x86_64 裸机 C 工具链 |
| 独立 QEMU CLI | `/opt/homebrew/bin/qemu-system-x86_64`、`qemu-img` 均为 11.1.2，原生 arm64；版本 / 能力枚举与动态链接核验通过 | 工具核验见第 2.8 节；T02 已建立真实 guest 串口入口，验证见第 2.11 节 |
| UTM | `/Applications/UTM.app` 4.7.5；内置 QEMU 与 qemu-img 动态库含 10.0.2 版本字符串 | `file` 确认是 Mach-O shared library，并非可直接运行的 CLI；版本来自静态字符串，未执行 guest |
| UTM 固件 | `Contents/Resources/qemu/edk2-x86_64-code.fd` 与 `edk2-i386-vars.fd` | 配对 JSON 声明 x86_64、pc-q35-*；没有启动证据，不能把 ARM 固件代入 |
| Linux 服务 | OrbStack 2.2.4；Docker context 为 orbstack，CLI / server 均为 29.8.2，服务为 linux / aarch64 | 初次盘点只读 API 成功；已完成固定 Debian 的 apt / sqv 验证，并构建本地工具镜像；五包仅装在派生镜像，任务容器均已清理 |
| 镜像 / 引导工具 | PATH 未发现 sgdisk、gdisk、mkfs.fat、fsck.fat、mformat、mcopy、mdir、xorriso、limine | 宿主搜索不是全机不存在的断言；基础镜像未含五包；派生工具镜像已安装五个精确版本并通过 7 个 CLI 命令，见第 2.6 节；Limine 12.9.4 资产已按第 2.11 节引入 |
| 系统辅助工具 | hdiutil、diskutil、newfs_msdos、fsck_msdos、file、shasum 存在 | 不用这些存在性证据替代规划中的固定 Linux GPT/FAT 工具环境 |

Rust 可执行文件位于 `~/.rustup/toolchains/<toolchain>/bin/`；本轮直接调用已安装二进制及 rustup 的 installed 查询，未执行工具链安装或更新。官方说明 `x86_64-unknown-none` 有预编译组件，支持 no_std，使用 alloc 需提供 allocator；目标本身不强制 nightly。项目仍沿用固定日期 nightly 方向，具体依赖功能通过实际编译确认。[Rust target 说明](https://doc.rust-lang.org/rustc/platform-support/x86_64-unknown-none.html)

UTM 的固件 JSON 保留上游构建机路径，不能原样复用。以下 SHA-256 是本机文件实测，仅用于识别此次盘点样本，不代表项目选择了这些固件：

| UTM 文件 | 字节数 | SHA-256 |
| --- | --- | --- |
| edk2-x86_64-code.fd | 3653632 | `d23e35c96cbf47468e5c7c2ce82cfa0c825d4e916bb6bacbf3d8d81556350fe3` |
| edk2-i386-vars.fd | 328704 | `a5c722949ba13c60b08c1ad3507ef751424941781f66d529f5f1feded8320ccb` |

## 2. 精确候选与已验证工具环境

以下记录候选与实际验证范围：Debian 工具镜像已完成第 2.6 节的安装 / 可运行性核验，QEMU 已由用户按正常 Homebrew 方案安装并完成第 2.8 节核验，指定 Rust nightly 已安装并完成第 2.9 节工具核验；它们都不是已冻结的产品依赖。版本号、包修订和架构一并核对；安装时如果来源内容变化、指定版本不可用或校验不符，应报告差异并调整方案，不能自动切换 latest。Rust / Debian 元数据于 2026-10-14 核对；正常 Homebrew QEMU 候选于 2026-10-19 重新核对并执行 dry-run。

| 层次 | 精确候选 | 来源与核对结果 |
| --- | --- | --- |
| Rust | `nightly-2026-10-13-aarch64-apple-darwin`，rustc `1.101.0-nightly (e9bbadf68 2026-10-12)` | 官方日期 manifest 已读取；安装后七项组件身份相符，rustc 实际版本与候选一致；T01 本地项目编译证据见第 2.10 节 |
| Rust 安装集 | minimal profile + rust-src、rustfmt、clippy；target 为 `x86_64-unknown-none` | 已安装宿主及裸机两个 target，使用自带 rust-lld；本次安装集不包含额外 llvm-tools、GDB 或交叉 GCC |
| QEMU | Homebrew `qemu 11.1.2`，formula revision 0、bottle rebuild 0，`arm64_golden_gate` | 官方 formula API 提供匹配 macOS 27 ARM64 的 bottle；已装版本与收据匹配，来源身份及哈希见下表及第 2.8 节 |
| x86_64 UEFI | 上述 QEMU 11.1.2 包内的 `edk2-x86_64-code.fd` + `edk2-i386-vars.fd` | QEMU 对应 tag 的 README 标明固件源为 `edk2-stable202302`；已按安装包文件、descriptor 与实测哈希核对，见第 2.8 节；第 2.11 节已取得启动证据，不借用 UTM 文件 |
| Linux 基础环境 | `debian:trixie-20261005-slim`，`linux/arm64/v8` | registry index → ARM64 manifest 映射与响应哈希已核对，见第 2.1 节；已按该 digest 拉取并执行第 2.5 节验证 |
| GPT | Debian trixie ARM64 `gdisk=1.0.10-2+b1` | 提供 sgdisk；不能误用该包 amd64 的 `1.0.10-2` 修订 |
| FAT 格式 / 检查 | Debian trixie ARM64 `dosfstools=4.2-1.2` | 提供 mkfs.fat、fsck.fat |
| FAT 文件访问 | Debian trixie ARM64 `mtools=4.0.48-1` | 提供 mcopy、mdir 等，可操作普通镜像文件 |

Rust manifest：[固定日期清单](https://static.rust-lang.org/dist/2026-10-13/channel-rust-nightly.toml)。安装前网页工具未能读取该资源，后经只读 HTTPS 文本请求成功解析；2026-10-19 重核清单与同源 SHA 一致。工具链后由用户安装，本轮不重新下载归档。

QEMU：[formula API](https://formulae.brew.sh/api/formula/qemu.json)、[formula 源码](https://github.com/Homebrew/homebrew-core/blob/master/Formula/q/qemu.rb)、[固定 release 的固件来源](https://github.com/qemu/qemu/blob/v11.1.2/pc-bios/README)、[x86_64 配对 descriptor](https://github.com/qemu/qemu/blob/v11.1.2/pc-bios/descriptors/60-edk2-x86_64.json)。仅用包版本不能推断固件已兼容；安装后文件哈希已记录于第 2.8 节，实际启动结果仍未取得。

Linux：[Official Images 标签声明](https://github.com/docker-library/official-images/blob/master/library/debian)、[ARM64 构建产物索引](https://github.com/debuerreotype/docker-debian-artifacts/blob/dist-arm64v8/trixie/slim/oci/index.json)、[gdisk](https://packages.debian.org/trixie/gdisk)、[dosfstools](https://packages.debian.org/trixie/dosfstools)、[mtools](https://packages.debian.org/trixie/mtools)。基础镜像标签不锁定后续 apt 软件源；正式工具镜像须同时记录软件源快照、完整 dpkg 清单和最终镜像 digest，不能仅靠这三项顶层版本宣称传递依赖已经锁定。

| 对象 | 元数据提供的 SHA-256 / digest |
| --- | --- |
| Rust 宿主完整发行归档（供识别，不表示 minimal 会下载整个归档） | `b3344b44dd3af7a6864c1156a07b40610cda3b6eaa9bbecb89cb7f91e9c3e2d3` |
| Rust 裸机 target 的 xz 归档 | `316173a4802efa9e6a87e37e9678019a66ef5da7b1e51d85b0e83f8bd74583d0` |
| QEMU 11.1.2 source tar.xz | `731b5681e4bb18be313231579b8efd0296c5b015fa36dc533874b639ba838016` |
| QEMU arm64_golden_gate bottle | `a399faaf2f8fb45d3b1f35704041a9104f7f717e9fda1b12d4deac9e919d0d33` |
| Debian ARM64 构建产物 manifest | `sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850` |

表内 Rust 哈希来自官方文本元数据；工具链后由用户安装，本轮核对已装 manifest、CLI 与文件，不声称独立复验了原始发行归档。QEMU 后由用户安装，本轮只核对已装文件 / 收据 / SBOM，不声称重新下载并校验了原始 bottle 归档。Debian 日期标签映射已补齐，并已按下述 ARM64 digest 拉取与运行任务验证容器；具体运行证据见第 2.5 节，不把原先的元数据核对本身当成执行证明。

QEMU formula 原文现已取得并计算内容哈希，与 API 的 `df8fe626603a12e042c0b33141343a3c36f059990addff1be4db4479fabe3561` 一致；固定来源见第 2.3 节。普通 `brew install qemu` 会随 formula 变化，不能视为永久精确安装命令。

### 2.1 Debian registry 映射与工具包影响

通过 Docker Hub 的匿名 registry 读取日期标签 index，再按平台 descriptor 读取 ARM64 manifest；两份响应均重新计算 SHA-256，与 registry 标识 / descriptor 一致。映射核对时只读取 JSON 元数据；后续经明确授权拉取镜像层并运行验证，见第 2.5 节。

| 证据 | 精确值 |
| --- | --- |
| 标签 | `docker.io/library/debian:trixie-20261005-slim` |
| 多平台 index | `sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f` |
| 选定平台 | `linux/arm64/v8` |
| 选定 manifest / 后续拉取身份 | `docker.io/library/debian@sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850` |
| 上游构建来源 revision | `cf1f4a45447842b45e9952e0f018ae734a7341c7` |
| layer 元数据 | 30,200,282 字节；`sha256:bbeda6b4abb749f743f4547ef5a28070a14de8caef211daa5afb78d5b5fa21ba`；Docker 按固定 manifest 拉取完成 |

来源为 [registry 日期标签](https://registry-1.docker.io/v2/library/debian/manifests/trixie-20261005-slim)与 [ARM64 构建包清单](https://github.com/debuerreotype/docker-debian-artifacts/blob/cf1f4a45447842b45e9952e0f018ae734a7341c7/trixie/slim/rootfs.manifest)。registry 查询需要匿名 pull scope token，token 不写入文档或日志。

对照基础包清单与已验证的 `20261005T000000Z` trixie main ARM64 Packages 索引，按 `--no-install-recommends` 推导以下五包新增；基础 libc6、libgcc-s1、libstdc++6、libtinfo6、libuuid1 等已满足这些包的必需版本。此元数据推导先由第 2.5 节的实际 apt 模拟求解确认，再由第 2.6 节的独立工具镜像构建完成实际安装；原基础镜像没有被修改。构建的版本 / 架构 / 大小 / 包体哈希输入以 [packages.tsv](../../tools/debian-image/packages.tsv) 为准，下表为对应说明。

| 新增包 | 精确版本 | 包下载字节 | Installed-Size（KiB） | 包 SHA-256 |
| --- | --- | --- | --- | --- |
| gdisk | 1.0.10-2+b1 | 216476 | 1089 | `b6b48390e63bff9d18b32be3270910fab83f8c14117d49ada13cba8a2a75c7e9` |
| dosfstools | 4.2-1.2 | 130564 | 374 | `7ec5c86659561b5e1b412227e0e3fab9dc17e4214274cb1b4cb3b5c5f8c288e9` |
| mtools | 4.0.48-1 | 192396 | 504 | `296cf0a6b06433520effc30542a695307f4abf0d377f94d8a22dc8bd1dd4785f` |
| libncursesw6 | 6.5+20250216-2 | 124048 | 568 | `72121aeb08be2d915da321ddfae74f62a0764286f336279b2c2609ba07496d6d` |
| libpopt0 | 1.19+dfsg-2 | 42808 | 257 | `b58ba178effc24280c4c6be3cf2c93a34607e18ad76b2f74b2de818136699076` |

五包合计 706,292 字节下载、2,792 KiB 安装大小；不含基础镜像、包索引、缓存和容器存储开销。groff-base、libgpm2 属 recommends，floppyd 属 suggests，不纳入该安装集。五包在快照 main 索引中的版本、架构、大小和 SHA-256 均与上表一致；其快照 `.deb` 路径逐项 HEAD 返回 200，Content-Length 与表内一致。早期 HEAD 仅证明路径可用；第 2.6 节已实际下载全部五个 `.deb`，逐项核对字节数、SHA-256 和包身份后才安装。

#### 快照签名与索引证据

基础镜像默认使用移动的 trixie / trixie-updates / trixie-security 源。选择上游 [snapshot 源模板](https://github.com/debuerreotype/docker-debian-artifacts/blob/cf1f4a45447842b45e9952e0f018ae734a7341c7/trixie/slim/rootfs.debian-sources-snapshot)的 `20261005T000000Z`；main / updates 使用 `archive/debian`，security 使用 `archive/debian-security`。固定基础镜像 digest 不能代替此软件源冻结。

本机未发现 gpg / gpgv；没有为核验安装它们。临时核验程序按 OpenPGP v4 格式解析实际的 RSA 签名，用已有 Python cryptography 验证 RSA PKCS#1 v1.5；校验主钥指纹、签名子钥绑定、子钥交叉认证与子钥到期时间，再验证三个 InRelease 的规范化正文。另用已有 OpenSSL 3.6.4 的 `dgst -sha256 -verify` 对提取的三个签名独立调用复核，均返回 `Verified OK`。正文改动和签名位翻转的负向样本均被拒绝。未执行下载的 formula 或安装脚本。

信任锚来自 Debian 官方 HTTPS 公钥文件，主钥 / 子钥指纹同时对照 [FTP master 公钥页](https://ftp-master.debian.org/keys.html)和 [2025 年公钥公告](https://lists.debian.org/debian-devel-announce/2025/04/msg00001.html)。这项早期证据是固定签名的密码学复核，**本身不等于 apt / sqv 的策略验证**，也没有建立离线预置信任锚或核验所有并列签名。后续已在固定镜像内使用 debian-archive-keyring 2025.1、apt 3.0.3、sqv 1.3.0-3+b2 完成实际验证，见第 2.5 节；早期临时解析程序没有进入 APT 信任路径。

| 公钥用途 | 主钥指纹 | 已验证签名子钥指纹 | 公钥文件 SHA-256 |
| --- | --- | --- | --- |
| trixie / updates | `04B54C3CDCA79751B16BC6B5225629DF75B188BD` | `B8E5F13176D2A7A75220028078DBA3BC47EF2265` | [archive-key-13.asc](https://ftp-master.debian.org/keys/archive-key-13.asc)：`6f1d277429dd7ffedcc6f8688a7ad9a458859b1139ffa026d1eeaadcbffb0da7` |
| trixie-security | `5E04A1E3223A19A20706E20F9904613D4CCE68C6` | `89C87ACEA5DD6B8E6A7068808E9F831205B4BA95` | [archive-key-13-security.asc](https://ftp-master.debian.org/keys/archive-key-13-security.asc)：`844c07d242db37f283afab9d5531270a0550841e90f9f1a9c3bd599722b808b7` |

两个子钥的绑定签名分别声明到期于 2035-03-28 12:50:29 / 12:51:41 UTC；所读公钥文件未含主钥或子钥撤销签名。每个 InRelease 只需此处相应的一个已核验 Debian 13 签名建立本次证据，其他并列签名不记作通过。

| 快照 suite / InRelease 来源 | InRelease 字节 / SHA-256 | main/binary-arm64/Packages.xz 字节 / SHA-256 |
| --- | --- | --- |
| [trixie](https://snapshot.debian.org/archive/debian/20261005T000000Z/dists/trixie/InRelease) | 140421 / `0584fba32e13e0ab8285fb16c27adea1ec03a73669c18702821094fd6ca86675` | 9614192 / `be16b37b2f740f066ea93d35d56f04f94d63693e5a3eb421b052bf86fae75bcd` |
| [trixie-updates](https://snapshot.debian.org/archive/debian/20261005T000000Z/dists/trixie-updates/InRelease) | 47340 / `ef1bab53128b164c03c968625d0f2e8841e3e754ff358ab010ccc2c7dbf6124a` | 4424 / `fe771d58d4af39dbb94142d6f3ce578d340c6b1364c172d9e87f2287bdc4994b` |
| [trixie-security](https://snapshot.debian.org/archive/debian-security/20261005T000000Z/dists/trixie-security/InRelease) | 43409 / `1a25f10a8e266ba293823ac2c52325b7333e49dddbc2ef74de71f8d7c6d6b7f8` | 264940 / `0a6cb15941f175daed3a39cb59228207d856749ab866e591b7e02e9bce8e1726` |

三个压缩索引均按已签名 Release 的 SHA256 段校验字节数和哈希；解压后的 Packages 也分别匹配 Release 声明的 `f97011263173eaccb1a215a28a6ec7ba1f6e5cebd8bb68e8fa0475b1d4021161`、`860a644dcb3ccd96a058f6c7717488f3d88ed42b0a53a6eac3ced6adf88e853b`、`7366882e60e70662a059c63b836d30ac140560de046080c2ae6dfeff1deae4fd`。main 索引与前轮当前源恰好相同，但本轮证据已来自固定快照及其签名，不再依赖移动源内容不变的假设。

trixie Release 的 Date 为 2026-09-12 07:55:41 UTC，没有 Valid-Until；updates / security 的 Date 分别为 2026-10-04 20:10:33 / 22:05:35 UTC，Valid-Until 分别为 2026-10-11 的同一时刻，核对当日已过期。签名正确不代表有效期检查通过。后续历史重现须仅对这组固定 snapshot 源设置 `Check-Valid-Until: no`，保留明确的 Signed-By、签名与包哈希校验；不全局禁用检查，不使用 trusted=yes / AllowInsecureRepositories，也不静默切回移动源。[snapshot 使用说明](https://snapshot.debian.org/#usage)、[APT 源配置](https://manpages.debian.org/trixie/apt/sources.list.5.en.html)

五个新增包没有声明 Pre-Depends、Conflicts 或 Breaks；必需 Depends 逐项检查后，已有 libc6 2.41-12+deb13u4、libgcc-s1 / libstdc++6 / gcc-14-base 14.2.0-19、libtinfo6 6.5+20250216-2、libuuid1 2.41.5-0+deb13u1 满足版本限制，包括 libncursesw6 对 libtinfo6 的精确等号要求。新增包所需的这些基础依赖递归仍由既有基础集合满足。updates 没有这五包，security 也未提供它们的其他候选；元数据结论为新增五包、无需因这五包升级基础库；第 2.5 节的实际 apt 模拟事务已得到同样结果。

固定基础包清单 SHA-256 为 `d36f1afce31468d16ff66da178751c73bfd5347271f8411015f8b937cd3b1132`。其中没有 ca-certificates，不能把宿主 HTTPS 查询成功推成容器 HTTPS 已可用；实际构建须遵循已审阅源模板的传输方式与签名校验，若改用需要新增 CA 包的方式，应重新记录依赖影响。新增包须纳入安装清单与授权范围。

### 2.2 正常 Homebrew QEMU 安装基线与预演

2026-10-19 按用户要求改用现有 `/opt/homebrew/bin/brew` 与官方 `homebrew/core`，安装到共享 `/opt/homebrew`。不再实施专用 `/opt/rrt01`、固定 tap 或第二套 Homebrew / portable Ruby；此前方案仅保留在月度日志与 Git 历史。本次安装基线为 **11.1.2（revision 0、bottle rebuild 0）**，运行依赖跟随本次官方 formula 解析，包含 **libssh 0.12.2_1 → openssl@4 4.0.3**。这不是原 OpenSSL 3 固定集合，也不是永久冻结的安装锁。

先核对本机 Homebrew 提交 `b3625f73d3e3574c5789ee32eb7b06627b788ec4`、所需目录和已有 portable Ruby 4.0.6_2，再实际执行 `/opt/homebrew/bin/brew install --formula --dry-run --no-ask qemu`，退出 0。命令级设置 `HOMEBREW_NO_AUTO_UPDATE=1`、`HOMEBREW_NO_ANALYTICS=1`、`HOMEBREW_NO_INSTALL_CLEANUP=1`，缓存 / 日志 / 临时目录放在本任务忽略目录；保留官方 API 解析及既有依赖检查，不改 HOME 或全局配置。预演下载了 API / bottle manifest 文本元数据，没有下载软件归档或执行安装。

实际计划为 **QEMU 本体 + 22 项新增依赖、4 项依赖升级、3 项既有依赖复用**，共 30 项。30 份官方 API 与本机 `brew info --json=v2` 的版本、依赖及目标 bottle 身份一致，运行图为 30 节点 / 39 条引用。下表记录安装前状态与预演；安装后的 30 项活动版本均与候选相符，实测见第 2.8 节。“复用”不表示既有 keg 与当前 bottle 字节相同或已通过全部功能兼容验证。

| 包 | 安装前 → 候选版本（含 formula revision） | 预演动作 | bottle rebuild / SHA-256 |
| --- | --- | --- | --- |
| ca-certificates | 2026-08-13 → 2026-09-25 | 升级 | 0 / `81804ad8a70a7a8d2a0a964d106765ff88716b310decaeffb4b4895a3252186a` |
| capstone | 未安装 → 5.0.9 | 新增 | 0 / `6b31fe379681e2c0778e30b3516cdfc2cfdff6424f666463643f1507039daf0b` |
| dtc | 未安装 → 1.8.1 | 新增 | 0 / `92ee4960b1643f3ba722d3d0cd9eb31af546215c7eaf858e76e2c69dd2c7afe8` |
| gettext | 1.0 → 1.0 | 复用 | 2 / `4db034aa2ae4b1f18d86906707ece14c895198090ad028d2fb1ae4fbeb3ca6fd` |
| glib | 未安装 → 2.90.1 | 新增 | 1 / `5ccb7a549258ed35cddc280882a11a854bd0ef1b2329a1ce77e43e39d2d98e3a` |
| gmp | 未安装 → 6.3.0 | 新增 | 1 / `5116a8201d8e044aef7d486da7e57f1e4dd2d8554ea6b540b44e45c0804f8f6f` |
| gnutls | 未安装 → 3.8.13_2 | 新增 | 1 / `366c9b9c0a6b246af58952c4bb5ac921ec5be87bdc23c0b9d8b2ca97b1a3d996` |
| jpeg-turbo | 未安装 → 3.2.0 | 新增 | 1 / `7bc0f7c007a73c68da8c11055e836de66c2814c1b0ca0302da9a6317199fd37a` |
| json-c | 未安装 → 0.19 | 新增 | 1 / `6b45746f31780a84ae3ed11d056bd101d1b0a6d0f16bf91aad2d35a18f6107dc` |
| libidn2 | 未安装 → 2.3.8 | 新增 | 0 / `bacc89595e4e8d2a4e14eb71730ae96a1ca68830068a741c59fc363a355ef1bc` |
| libpng | 未安装 → 1.6.59 | 新增 | 0 / `7e04225ac0bc048d2b12c603f678e5fa36459909812f2f1306dbd97af337cdec` |
| libslirp | 未安装 → 4.9.5 | 新增 | 0 / `9c0233380116a46e7d292a7f0b1b3084214f399ae839806bcff0018f581da405` |
| libssh | 未安装 → 0.12.2_1 | 新增 | 0 / `48110039ef444f383b5fab938dffbca01203784eafcf2a05135477cffef15751` |
| libtasn1 | 未安装 → 4.21.0 | 新增 | 0 / `68d1748171848565af7a083ca92b834ab1fe4ac7832140b1cd22fd1fcb072fd7` |
| libunistring | 1.4.2 → 1.5 | 升级 | 0 / `fec6be46cabaefd1dc5d5ecd029c50aeaef6c93d9c7de4425968047811575e81` |
| libusb | 未安装 → 1.0.30 | 新增 | 0 / `b462926c2314b5bb7d1a6721f9a3cf7336a4166ba67024b4bbd0226de457132a` |
| libyaml | 0.2.5 → 0.2.5 | 复用 | 0 / `afaaa073d70a7e69f0ff53dcdf1ee08d1ac594eb2e458afa665403adf4209078` |
| lz4 | 未安装 → 1.10.0 | 新增 | 3 / `53ea7532077fe43abfe3d6e0d9991ebc8370e9edcc508f997c92902d60c8b151` |
| lzo | 未安装 → 2.10 | 新增 | 0 / `928ed0070e25fe8082e24daa3f967657be0c1c5c72c14a3ac4cd498874eeec11` |
| ncurses | 未安装 → 6.6 | 新增 | 0 / `36a1f053f1be8ce8dce8a92b68395fb1483b077d313077e9c8f4b20b6f712602` |
| nettle | 未安装 → 4.0 | 新增 | 0 / `42ff85a054b4a265542175dd31f4a515e0a3e7d8113cfddacc5472555a036bc8` |
| openssl@4 | 未安装 → 4.0.3 | 新增 | 0 / `cfe1f3ee8ddafdb7f19fe4d4dd600e2b84257df4fa09a169ccb7322d38483f90` |
| p11-kit | 未安装 → 0.26.5 | 新增 | 1 / `a7c4679040141c7c8cb0861e164126871f7e0eb485df6da5ebecc3665e185ce2` |
| pcre2 | 10.47_1 → 10.49 | 升级 | 0 / `5cac44323243138490ca74a2499fd347fa836df6b5fffa789944be12110e91ff` |
| pixman | 未安装 → 0.46.4 | 新增 | 1 / `918ff532bf70bc121376f4b51420f44fc1abd0b9c519d7f8040f6ad9d304bc02` |
| qemu | 未安装 → 11.1.2 | 新增 | 0 / `a399faaf2f8fb45d3b1f35704041a9104f7f717e9fda1b12d4deac9e919d0d33` |
| snappy | 1.2.2 → 1.3.1 | 升级 | 0 / `7f20ac629eaf9a34a0e4d0e1fe60a933d42ececf3d956988fa8b24916e467c85` |
| vde | 未安装 → 2.3.3 | 新增 | 0 / `972ce0f14fb0fe92932f45b887019b228ac4529f06d0a95eeb88ece1359d76ba` |
| xz | 5.8.4 → 5.8.4 | 复用 | 0 / `b2b5d6523153be05c20bcc51cd358ea3dd1f4bf893748fdedb779ab508ea63ce` |
| zstd | 未安装 → 1.5.7_1 | 新增 | 0 / `7fb4af03304cc4897326168cdcd53290311c5543150f5502a61c3e84b6d62765` |

除 ca-certificates 使用 `all` 外，目标平台均为 `arm64_golden_gate`。复用三项的 bottle 哈希仅识别当前候选，运行身份以已有收据与安装后实际链接为准。ca-certificates 还保留旧 keg 2026-05-14，安装前 opt 指向 2026-08-13，安装后为 2026-09-25；旧 keg 保留，见第 2.8 节。当前集合没有安排升级 / 移除 openssl@3，也没有 pin 阻止上述四项升级。

普通 Homebrew 会随上游和本机状态变化；执行前须重新预演并比较本表。`HOMEBREW_NO_AUTO_UPDATE` 只控制命令自动更新行为，不构成依赖锁。预演不验证真实 pour、链接、post-install 或所有反向依赖；实际安装发现损坏链接时仍可能触发相关依赖升级 / 重装，不能屏蔽检查来强行维持 23 / 4 / 3。出现额外动作、版本 / 哈希变化或源码构建需求时，先列明差异并更新影响范围。[Homebrew install / dry-run 说明](https://docs.brew.sh/Manpage#install-options-formulacask-)

### 2.3 共享依赖与其他项目影响

本轮保存的官方 API 指向 homebrew-core 提交 `d929e58761d4e047dbb728b514fae89f9eef1cf1`。另读取以下七份固定提交原文，重新计算内容 SHA-256 与 API 对照；没有执行配方或 post-install。其余候选的来源路径 / commit / SHA 保存在本地 API 证据中。

| 官方配方原文 | SHA-256 |
| --- | --- |
| [qemu](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/q/qemu.rb) | `df8fe626603a12e042c0b33141343a3c36f059990addff1be4db4479fabe3561` |
| [libssh](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/lib/libssh.rb) | `77d11f59dc8fbf677e0be68be9a7711f1433e5d238ea02a3e4bbc759392ee8ba` |
| [openssl@4](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/o/openssl@4.rb) | `9e6998cc58157aa31b16144b11b67d686caccd3032d7882a76eccb53ce0730f7` |
| [libunistring](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/lib/libunistring.rb) | `e19b8847465634aa2ab365c7fa641216450ad26cc02bddfce08c8d17342d21dc` |
| [pcre2](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/p/pcre2.rb) | `2dcb74b4bc19dc81ace51feac7bb626b679c7b730eca1d3492d73fe822134fb8` |
| [ca-certificates](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/c/ca-certificates.rb) | `e7807634f02d41cfad036cdebb546daacd45e118fcb463acc6c6e4f6e07dcf3f` |
| [snappy](https://github.com/Homebrew/homebrew-core/blob/d929e58761d4e047dbb728b514fae89f9eef1cf1/Formula/s/snappy.rb) | `65ad62d494f32ebad469606114480e5aeb6e88cac51d6bd5067e7cfed4c86959` |

**OpenSSL 4 的影响不止新增一个并存目录。** 当前 formula 不是 keg-only，明确允许覆盖共享 `bin/openssl`、OpenSSL 头文件、`libssl*` / `libcrypto*`、pkg-config 与 CMake 的 OpenSSL 发现入口。安装前 `bin/openssl`、`include/openssl`、`lib/libssl.dylib`、`lib/libcrypto.dylib` 和 `lib/pkgconfig/openssl.pc` 均指向 openssl@3 3.6.4；安装后已核对默认 CLI 与这些共享入口转向 OpenSSL 4.0.3。`/opt/homebrew/opt/openssl@3` 及其版本化 dylib 可以保留，但不能据此声称共享默认不变或所有项目无影响。

| 变化 | 已观察到的使用路径 | 实际影响与后续验证 |
| --- | --- | --- |
| libunistring 1.4.2 → 1.5 | gettext 1.0 的收据与辅助 dylib 指向它；Git 收据间接包含 gettext | gettext 相关功能需验证；不能仅由收据推断每个 Git 命令必经该库 |
| pcre2 10.47_1 → 10.49 | Git 2.54.0、ripgrep 15.2.0 的实际 Mach-O 直接链接共享 opt/pcre2 | 两工具会加载升级库；需用合成仓库 / 文本检查相关正则功能 |
| snappy 1.2.2 → 1.3.1 | leveldb 1.23_2 直接链接；librime 1.17.0 经 leveldb 使用 | RadishLex 使用共享 librime 的本机开发 / 重新打包路径受影响；需合成数据的原生库功能验证 |
| ca-certificates 2026-08-13 → 2026-09-25 | OpenSSL 3、Ruby、CocoaPods、FFmpeg 的收据依赖链；OpenSSL 3 的 cert.pem 指向共享 CA 文件 | post-install 重建 `/opt/homebrew/etc/ca-certificates/cert.pem`，可能改变现有 TLS 信任结果；保留 OpenSSL 3 不隔离此影响 |
| 新增 openssl@4 4.0.3 | 共享 CLI / 编译入口；RadishLex 脚本调用未限定路径的 `openssl cms` | 默认命令行为、算法 / provider 和构建发现需单独核对；未实测不能承诺兼容 |
| 新增 glib / gnutls / openssl@4 | API 声明 post_install_steps | glib 创建 `lib/gio/modules`，gnutls / OpenSSL 4 建立各自 etc 下的 CA 链接；均会写共享 prefix；本轮只核对安装结果，不重跑 post-install |

ca-certificates 的 macOS post-install 会读取 `/Library/Keychains/System.keychain` 与 `/System/Library/Keychains/SystemRootCertificates.keychain` 中的证书，结合 Mozilla CA 生成上述 bundle；源码不读取登录钥匙串或私钥。安装前只审阅源码；安装后仅核对 bundle 的存在、大小、摘要及引用链接，未执行 `security` 或摘录证书内容，也未观测用户安装期间的 post-install 过程。不能以 bottle 哈希保证最终信任集合相同；升级后的 TLS 行为仍未验证。

其他项目核对限于相关源码和已安装二进制的 `otool -L`，未修改或运行兄弟项目：RadishLex 的 `crates/ime-engine-rime/build.rs` 使用 `RIME_INCLUDE_DIR` / `RIME_LIB_DIR` 或 pkg-config 的 rime；`scripts/check-macos-upgrade-product-coordination.sh` 会取 `brew --prefix librime`；`scripts/build-manager-macos-dpk-qualified-product.sh` 使用未限定路径的 `openssl cms -verify`。其 `scripts/macos-imk/bundle_dylibs.py` 会复制动态库并改写包内引用，因此不能推断已打包应用会自动加载升级后的共享库；具体制品仍需独立核对。

FFmpeg 的已安装二进制明确指向 `opt/openssl@3` 的版本化 dylib，不会仅因默认 `openssl` 切换就自动改用 ABI 4；但共享 CA 更新仍需考虑。其他项目通过 Git、ripgrep 或共享 TLS 间接受影响的范围取决于实际调用路径。本次不是所有项目的完整兼容性审计，也没有取得升级后的功能证据。

### 2.4 正常 Homebrew 安装与失败处理方案

以下保留正常安装 / 后续重装的操作边界。用户已完成本次安装并选择默认使用 OpenSSL 4；本轮仅做第 2.8 节安装后核验。需要 OpenSSL 3 的项目在各自命令或构建入口明确指定其专用路径，不全局切回，也不以 PATH 设置替代编译库路径选择。

1. **复核实际范围。** 在现有 `/opt/homebrew` 重做版本 / 来源核对和 dry-run，确认本表的候选及新增 / 升级动作。若默认 OpenSSL 入口必须保持 3，则当前正常安装方案不满足该约束，应先调整方案；不临时加入强制链接、自定义 tap 或依赖跳过。
2. **保存安装前状态。** 记录全部已装收据、相关 opt / linked-keg / bin / lib / 头文件及构建发现链接、受影响工具版本；在安装授权范围内保留受影响旧 keg、配置和 CA bundle 的可恢复副本，私密材料不进入仓库或日志。仅设置本次命令不清理旧版本；不运行全局 upgrade / cleanup / autoremove，不把保留旧 keg 当作完整回滚。
3. **按正常依赖安装。** 使用现有 `/opt/homebrew/bin/brew install --formula qemu`，命令级保留第 2.2 节的自动更新 / 清理控制及官方依赖检查。只接受已审阅的 bottle；若需要源码构建、额外包、移除既有包或修改全局设置，停止并报告范围变化。正常安装会写共享 Cellar、opt、bin、lib、include、share 和 etc，并可能调整反向依赖；不可按专用环境的“共享零变化”验收。
4. **核对实际结果。** 逐项比较收据 / 链接差异、真实版本及库来源，记录 OpenSSL 3 / 4、默认 CLI 与编译发现、CA bundle 的结果；验证 Git / ripgrep 及相关原生库的代表性合成用例。兄弟项目运行或修改按对应项目授权处理。本项目完成第 4 节的 QEMU CLI、TCG、机器型号和固件检查后，才取得工具可运行证据；guest 启动仍属后续验证。
5. **失败保留现场。** 保存失败阶段、日志、已完成动作与共享状态差异，停止继续安装 / 清理。卸载 qemu 不会撤销四项升级、默认 OpenSSL 链接或 CA 更新；Homebrew 安装不是完整事务。恢复须逐项核对当前使用者与前置备份，形成针对实际失败的恢复操作，不直接回写旧收据、删除共享库或批量重链。没有演练过的恢复路径不声明可靠。

本方案复用用户日常 Homebrew，后续升级由其正常维护流程管理。RadishRoot 在每次验收中记录实际 QEMU、依赖、固件与机器配置身份；共享升级后重跑受影响验证，不能长期沿用旧运行证据，也不自动引入项目私有冻结环境。

### 2.5 Debian apt / sqv 验证方法与实测

以下方法独立于宿主 QEMU 安装。已于 2026-10-14 在用户明确授权的固定 digest 临时容器内完成，实测结果见本节末尾；只验证签名、历史快照有效期与模拟求解，不安装五个工具包。重现时仍需覆盖镜像拉取和容器执行的授权。

1. 仅拉取第 2.1 节 ARM64 manifest digest，记录实际平台、image ID 与容器 ID。使用任务专用临时容器和 `/work` 写入区，不使用 privileged、宿主设备、Docker socket 或其他项目卷；宿主只提供只读公开核验材料。核对镜像内 apt 3.0.3、sqv 1.3.0-3+b2、debian-archive-keyring 2025.1 和完整 dpkg 清单；保存 `/var/lib/dpkg/status` 哈希。版本或基础清单不符即停止。
2. **离线 sqv。** 在 network=none 的容器内先读 `sqv --help`，使用镜像自带的 `/usr/share/keyrings/debian-archive-trixie-automatic.gpg` 验 trixie / updates，使用 `debian-archive-trixie-security-automatic.gpg` 验 security。命令形式为 `sqv --cleartext --signatures=1 --keyring=<对应 keyring> --output=<新的 Release 输出> <InRelease>`。记录 UTC 时间、keyring 哈希、工具版本、退出码及签名者；使用默认当前策略，不回拨时间、不放宽证书策略。三个都成功后，按输出 Release 重新校验压缩与解压索引。另用独立副本测试正文篡改和错误 keyring，必须失败；不能覆盖原证据。[sqv 1.3 手册](https://manpages.debian.org/trixie/sqv/sqv.1.en.html)、[keyring 包文件清单](https://packages.debian.org/trixie/all/debian-archive-keyring/filelist)
3. **隔离 APT。** 新的网络阶段容器继续使用相同 digest；APT_CONFIG 指向 `/work/apt.conf`。将 `Dir::Etc::parts` 指向已创建的任务空配置目录、`main` 指向空配置文件，以隔离镜像额外配置 / hooks；sourcelist 指向 `/work/snapshot.sources`、sourceparts 指向任务空源目录，lists 为全新 `/work/lists`，archives 为 `/work/archives`，status 仍读 `/var/lib/dpkg/status`，pkgcache / srcpkgcache 为空。目录为 `_apt` 提供必要权限，不用关闭下载沙盒补救。用 apt-config dump 核对最终设置，不继承代理凭据或策略覆盖；记录输出时不包含秘密。不要把 parts 设为 `-`：APT 3.0.3 会尝试读取 `/etc/apt/-/` 并产生目录缺失警告。不同重跑使用新的任务子目录，不复用旧 lists。

源文件仅包含以下两个 deb822 stanza；HTTP 沿用已审阅 snapshot 模板，避免为基础镜像缺 CA 而新增包，真实性继续由 Signed-By 和索引 / 包哈希保证。`Check-Valid-Until: no` 只处理这组历史快照的已知过期；保留 Check-Date 和签名检查，不设置 Trusted 或允许不安全仓库。

```text
Types: deb
URIs: http://snapshot.debian.org/archive/debian/20261005T000000Z/
Suites: trixie trixie-updates
Components: main
Architectures: arm64
Signed-By: /usr/share/keyrings/debian-archive-trixie-automatic.gpg
Check-Valid-Until: no

Types: deb
URIs: http://snapshot.debian.org/archive/debian-security/20261005T000000Z/
Suites: trixie-security
Components: main
Architectures: arm64
Signed-By: /usr/share/keyrings/debian-archive-trixie-security-automatic.gpg
Check-Valid-Until: no
```

4. **实际 APT 更新与策略。** 执行 `apt-get -o APT::Update::Error-Mode=any -o Debug::Acquire::sqv=true -o Debug::pkgAcquire::Auth=true update`，要求退出 0，记录实际 sqv 验证链；需要方法协议细节时额外启用 `Debug::pkgAcquire::Worker=true`。本版本的 gpgv 调试开关不产生所需的 sqv 调用日志；不能出现未认证源、NO_PUBKEY 或下载失败后沿用旧索引。按第 2.1 节比对收到的三个 InRelease；若 APT 另存 lz4 等格式，用镜像自带 apt-helper cat-file 解码，再核对已认证的 Packages 哈希。另开全新 lists 目录，移除源内 Check-Valid-Until 例外，确认 updates / security 因过期而失败；不将该负向结果用于后续求解。[apt-get 手册](https://manpages.debian.org/trixie/apt/apt-get.8.en.html)、[APT 配置顺序](https://manpages.debian.org/trixie/apt/apt.conf.5.en.html)
5. **只模拟求解。** 先用 apt-cache policy 核对五包的版本与快照来源，再在成功更新的配置下执行下列命令。必须得到 0 upgraded、5 newly installed、0 removed，Inst 恰好是表内五个精确版本，不能有基础包升级或额外 recommends。输出即使退出 0，集合不符也判失败。

```sh
apt-get --simulate --no-install-recommends --no-remove install \
  gdisk=1.0.10-2+b1 dosfstools=4.2-1.2 mtools=4.0.48-1 \
  libncursesw6=6.5+20250216-2 libpopt0=1.19+dfsg-2
```

6. **收尾。** 再次核对 dpkg status 哈希和包清单未变、archives 内无下载的 deb，保存命令、退出码和必要公开输出；退出并移除本任务容器及专用临时数据，保留证据时记录位置。只删除归属明确的镜像 / 缓存，不做全局 prune。失败时修正明确原因后重跑对应步骤，不关闭签名、改用移动源或实际安装来“试出”答案。只有这套实际流程通过才能记录 apt / sqv 验证；工具包安装与制镜像另按授权执行。

#### 2026-10-14 实测结果

验证使用固定 manifest `sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850`；Docker inspect 返回 image ID 同值、`linux/arm64`，逻辑 Size 为 138,708,925 字节。初始本地不存在此镜像，本轮拉取成功。容器内 78 项完整包清单逐字匹配第 2.1 节的固定基础清单；用 `dpkg-query` 的 `${binary:Package}` 保留多架构后缀，不能用缺少后缀的 `${Package}` 误报版本差异。

| 检查 | 实际结果 | 证据边界 |
| --- | --- | --- |
| 组件身份 | apt 3.0.3；sqv 包 1.3.0-3+b2，程序 1.3.0 / sequoia-openpgp 2.0.0 / Nettle 3.10；keyring 2025.1 | 使用基础镜像自带组件，没有另装验证器 |
| 离线签名 | network=none；main / updates / security 的 sqv 均退出 0 | main / updates 输出 Debian 13 自动签名主钥指纹，security 输出对应安全主钥；均与第 2.1 节一致 |
| 签名负向 | 三份正文篡改、三次错误 keyring，共六项均退出 1 | 保留原始证据，负向副本未用于 APT 更新 |
| 索引链 | sqv 输出的 Release 验证三个压缩 / 解压 Packages 的大小与 SHA；APT 收到的三个 InRelease 与解码 Packages 再逐字匹配既有证据 | 未下载 .deb；不声明包体哈希实测通过 |
| APT 源策略 | 全新隔离 lists 的 update 退出 0，无 W:/E:、NO_PUBKEY 或下载沙盒降级；调用日志显示 `/usr/bin/sqv`，三次退出 0、三次 Signature verification succeeded | Signed-By 仅相应 Debian 13 keyring；保持不安全源禁止与日期检查 |
| 有效期负向 | 新 lists、仅 updates / security，去掉源内有效期例外后 update 退出 100；两源分别报 Release expired | 证明历史快照例外有必要；没有关闭签名或全局日期检查 |
| 五包求解 | `0 upgraded, 5 newly installed, 0 to remove and 0 not upgraded`；Inst 恰为第 2.1 节五包及精确版本 | apt-cache policy 均为未安装、候选来自固定 trixie/main；groff-base / libgpm2 未进入事务 |
| 无安装与清理 | 前后完整包清单相同，dpkg status SHA 相同，archives 无 .deb；两个任务容器最终退出 0 后删除，标签查询无残留 | 镜像缓存保留，无容器或后台验证进程；没有制镜像、QEMU 安装或业务实现 |

独立 sqv 在 2026-10-14 13:33:50 UTC 开始，使用默认策略；APT 成功事务在 13:36:08 UTC 开始。APT 补采调用日志显示其自行传入 `--policy-as-of 2027-10-14`，本任务没有覆盖该策略日期，也没有回拨时钟。受限 keyring 的独立 sqv 诊断包含缺少其他并列签名公钥的提示；指定的 Debian 13 签名已满足 `--signatures=1`，不能把未验证的其他并列签名记作通过。

| 容器内文件 | SHA-256 |
| --- | --- |
| `/usr/share/keyrings/debian-archive-trixie-automatic.gpg` | `8dbd0029697f8c9b009eeb9a153c6536b62ca031fa0a5b4cec74e8d718fccef6` |
| `/usr/share/keyrings/debian-archive-trixie-security-automatic.gpg` | `be1a7981908ab9010352131fcbf101a9556f6a61db76a19a1c146d31ef2d72d8` |
| `/var/lib/dpkg/status`（签名 / 求解前后相同） | `56d3fd5a297bdf152d897f3b39dd7d573d9941e0e69df3279e4ad5aada17cdb4` |

这组运行关闭了固定 Debian 基础环境的签名策略、过期拒绝和五包模拟求解缺口，当时没有安装工具包。后续独立派生镜像的实际安装 / CLI 验证见第 2.6 节；GPT/FAT 文件操作和 T01 裸机编译仍未进行。

### 2.6 本地 Debian 工具镜像：构建与验收

2026-10-14 已按授权构建并保留任务专用工具镜像。构建单一入口为 [Dockerfile](../../tools/debian-image/Dockerfile)，由它调用 [build-image.sh](../../tools/debian-image/build-image.sh)；没有 Cargo / guest 业务入口。固定输入包括基础 digest、[snapshot.sources](../../tools/debian-image/snapshot.sources)、[五包清单](../../tools/debian-image/packages.tsv)和[基础 78 包清单](../../tools/debian-image/base-packages.tsv)。五包清单每行依次为包名、版本、架构、字节数、SHA-256；基础清单逐字来自第 2.1 节固定上游 rootfs.manifest，SHA 仍为 `d36f1afce31468d16ff66da178751c73bfd5347271f8411015f8b937cd3b1132`。

构建先核对 ARM64 / 基础清单，再用隔离 APT 配置认证固定 snapshot，下载且只接受五个精确包体。五个大小 / SHA-256 及 deb 内的 Package / Version / Architecture 均通过后，才以 `--no-download --no-install-recommends --no-remove` 安装；失败立即退出，不补装额外包或改用移动源。构建后核对原 78 包版本未变、总计恰为 83 包、dpkg audit 无异常；镜像保留构建来源与证据，删除本构建的索引和包体缓存。最终默认软件源也指向固定 snapshot，签名与日期检查保持启用，仅源内保留历史有效期例外。

| 交付 / 核对项 | 本次实测 |
| --- | --- |
| 本地镜像标签 | `radishroot/debian-tools:trixie-20261005-arm64` |
| 本地 manifest list / image ID | `sha256:cd67b70bf3a9c2f064349e7283a73412e726b053d309dc36f1960600e745395b` |
| 平台与大小 | `linux/arm64`；Docker inspect Size 为 142,257,179 字节，不是新增宿主实际占盘测量 |
| 五包安装 | gdisk 1.0.10-2+b1、dosfstools 4.2-1.2、mtools 4.0.48-1、libncursesw6 6.5+20250216-2、libpopt0 1.19+dfsg-2；新增 5、升级 0、移除 0 |
| 包体 | 五个下载共 706,292 字节，全部 SHA / 大小 / 包身份匹配第 2.1 节；没有安装 recommends / suggests |
| 完整包清单 | 83 项，原 78 项名称和版本逐字保持；运行容器再次查询相同。清单 SHA-256：`500dc039d96076bc642b2c15cb61d6b174c8f2336b2cfb895d94aae0dba8ac8f` |
| 构建输入清单 | packages.tsv SHA-256：`9f12b981a31ad4a8483b3b773c276b46808f306b648ea3e60075c6383de5f7c8`；镜像中 recipe.sha256 的五份来源文件与仓库逐字相同 |
| 工具运行 | sgdisk --version 为 1.0.10；mkfs.fat --help 显示 4.2；fsck.fat --help 正常输出用法；mformat / mcopy / mdir / mtype 的 -V 均为 4.0.48；七条命令均退出 0 |
| 链接 / 包状态 | 七个可执行文件的 ldd 无 not found；dpkg audit 为空；运行前后 dpkg status SHA 均为 `ad0073442b403d2841e06c534d76f169a893ffc84023e1f6e561cf8a19a0ff33` |
| 清理与外部状态 | 专用验证容器退出 0 后按完整 ID 删除，任务标签查询无残留；工具镜像、基础镜像与 Docker 构建缓存保留，没有 registry push |

镜像内 `/usr/local/share/radishroot-debian-tools/` 保存完整 `packages.tsv`、`package-bodies.sha256`、`recipe.sha256`、dpkg 状态哈希、APT 配置 / 更新 / 安装日志与 `build/` 下的五份构建来源。包的 copyright 和公共许可证文件保留，来源登记见[许可与来源](../governance/licensing-and-sources.md)。这些工具仅用于本地开发，不进入 RadishRoot guest 镜像；本轮没有发布或作分发许可结论。

已执行的构建入口（仓库根目录，需相应 Docker 执行 / 下载授权）：

```sh
docker build --platform linux/arm64 --progress=plain \
  --tag radishroot/debian-tools:trixie-20261005-arm64 tools/debian-image
```

本次运行验证使用上述不可变本地 image ID；以下命令可读取完整清单，容器退出即删除，不挂载宿主目录：

```sh
docker run --rm --network none --cap-drop ALL \
  sha256:cd67b70bf3a9c2f064349e7283a73412e726b053d309dc36f1960600e745395b \
  cat /usr/local/share/radishroot-debian-tools/packages.tsv
```

标签可被后续构建更新，运行证据绑定本次 ID；固定输入不保证包含构建时间 / provenance 元数据的整个镜像逐字重建为相同 ID。镜像只在本机保留，Docker 返回的本地 RepoDigests 不证明 registry 上已发布。2026-10-14 的工具版本 / 帮助和动态链接通过仅证明可执行入口可用，当时未创建测试盘；后续 GPT/FAT 文件操作证据见第 2.7 节，QEMU 安装后核验见第 2.8 节。Rust 安装后工具核验见第 2.9 节，项目编译及产品 AT 仍未执行。

构建脚本仅供上述 Dockerfile 在固定容器内执行，不在 macOS 宿主直接运行。APT 更新、五包体核验、安装和完整清单对照是构建内检查；表中的七条 CLI 命令、ldd 和运行前后状态比较是本次构建后的独立验收，不由 Dockerfile 或现有 PR Checks 自动执行。修改配方后，应核对新镜像的固定输入与包清单，并在新 ID 上重新执行对应工具验收，不能沿用旧 ID 的成功记录。`sh -n tools/debian-image/build-image.sh` 只检查 shell 语法；`python3 scripts/check-repo.py` 只检查仓库治理，两者都不能替代真实构建。

### 2.7 GPT/FAT 普通文件操作与独立读回

2026-10-19 按授权复用第 2.6 节不可变镜像 ID，使用任务专用普通文件完成制盘、合成文件写入、独立读回和损坏检测。候选代码为 `ee6bcff7daad66cb780a9b1fb3bff5837254e4a2`；未修改镜像配方或安装依赖，没有 QEMU / guest、宿主设备透传或挂载真实磁盘。

本地证据位于仓库忽略目录 `evidence/t01-disk-20261019-ee6bcff/`：`produce.sh` 为制盘步骤，`verify.py` 为宿主 Python 标准库只读检查，`damage.py` 创建新副本并记录精确损坏偏移，`check-damage.sh` 调用已有工具只读检查；`report.txt`、JSON、工具日志及 `SHA256SUMS` 保存结果与材料身份。这些是本批有限样本的验证材料，不是通用 FAT 实现、产品制盘入口或现有 CI 覆盖；大文件与本地材料不提交 Git。

| 检查项 | 实际结果 |
| --- | --- |
| 整盘与 GPT | `disk.raw` 为 1,073,741,824 字节 / 512 字节扇区；独立核对保护 MBR、主备 GPT 头及分区数组 CRC、主备数组一致性、分区类型 / 名称 / 边界；通过 |
| ESP | LBA 2048–264191，128 MiB FAT32；空文件系统，未放入引导程序；FAT 结构与检查通过 |
| DATA | LBA 264192–2097118，共 1,832,927 扇区；mkfs.fat 实际文件系统为 1,832,922 扇区，分区末尾 5 扇区不属于 FAT；容量检查通过 |
| 布局提示 | sgdisk 提示 DATA 结束位置未按 2048 扇区对齐，可能影响部分磁盘加密工具；此布局用至 GPT 最后可用 LBA，主备 GPT 与分区无重叠。本批不改变 MVP 契约或冻结未来产品制盘参数 |
| 合成材料 | 空目录；1649 字节跨扇区文本；41 字节带空格名称文件；52 字节 UTF-8 名称 `萝卜.txt`；1 MiB 二进制模式文件。写盘前确定各自大小和 SHA-256 |
| 独立读取 | 制盘容器退出后，宿主标准库解析 FAT32、双 FAT、备份引导扇区、FSInfo 签名、簇链、短名 / LFN；四文件的名称、长度和原始内容哈希均与独立预期一致，空目录成立 |
| 写入边界 | mtools 写入前后 DATA 之外的两段字节（包含 ESP、主备 GPT）SHA-256 相同；正常镜像在故障测试后保持原哈希 |
| 包状态 | 制盘容器前后 83 包名称 / 版本逐字一致，两个容器 dpkg status 前后均未变；未安装、更新或移除包 |
| 正常镜像 SHA-256 | `fe3cc43bb11e88b84929c8b5bd7b712fc925f2e3c7b9b3749eb244d89a678583` |

五个故障案例均由正常盘生成独立稀疏副本；逐字节对照证明只有声明偏移发生改变。工具检测将材料只读挂载，另核对检测前后哈希，未修复或格式化损坏盘。

| 故障案例 | 独立检查（CLI 均退出 1） | 外部工具实际行为 |
| --- | --- | --- |
| 主 GPT 头 CRC 损坏 | 主头无效，备份有效 | sgdisk 退出 0，报告 Main header ERROR，并在内存中使用备份 |
| 主备 GPT 头 CRC 均损坏 | 两头均无效 | sgdisk 退出 0，输出 Creating new GPT entries in memory，随后 No problems found；不能据此接受原盘 |
| 主 GPT 分区数组损坏 | 主数组 CRC 无效，备份有效 | sgdisk 退出 0，报告 Main partition table ERROR 和 CRC 问题 |
| DATA 主备 FAT 引导扇区的扇区大小清零 | 拒绝 FAT 几何参数 | fsck.fat -n 退出 1，Logical sector size is zero |
| 合成文本首字节篡改 | 文件内容与预期哈希不符 | fsck.fat -n 退出 0；结构完整不证明内容正确 |

**验收不能只取 sgdisk 的退出码或末尾 No problems found，也不能用 fsck.fat 的结构检查代替内容哈希。** 正常盘独立检查退出 0；五个损坏盘独立检查均退出 1，并核对具体失败类别。独立读取器的故障结果与原始 GPT CRC、预先生成的材料摘要及外部工具诊断一起构成本批证据，不声称覆盖所有 FAT/GPT 损坏模式。

两个容器均使用 `--pull never`、无网络、只读根文件系统、cap-drop ALL、no-new-privileges、1 CPU / 256 MiB / 64 PID 上限；仅挂载本任务目录。容器均退出 0，按完整 ID 删除后，任务标签 `com.radishroot.task=t01-disk-20261019-ee6bcff` 查询无残留。保留原工具镜像、六份 1 GiB raw、五份分区 FAT 文件及日志；稀疏文件实际占盘约 50 MiB，逻辑容量合计约 10.33 GB，精确清单见本地报告。不保留后台资源。

这关闭了已有工具制盘、独立读取及本批故障检测的缺口；不证明内核驱动、flush、guest 重启持久化、引导或安装成立，T01、M0 与产品 AT 均未因此通过。

### 2.8 QEMU 正常安装后的实测核验

2026-10-19，用户报告已完成正常 Homebrew 安装，授权核对版本、动态链接、默认 OpenSSL 与固件并更新文档。本轮没有执行安装 / 升级 / 卸载、修改共享链接或启动 guest。以下证据来自已安装文件、收据和真实 CLI；没有用户安装全过程日志，不补写下载 / post-install 成功过程，也不把历史预演当作安装结果。

| 核验项 | 实际结果 | 证据边界 |
| --- | --- | --- |
| 可执行文件 | `/opt/homebrew/bin/qemu-system-x86_64` 与 `qemu-img` 均报告 11.1.2；`file` 为 Mach-O arm64；系统仿真器 `codesign --verify --verbose` 退出 0 | 证明本机工具可运行及签名结构校验通过，不等于产品引导通过 |
| 安装版本与来源 | 第 2.2 节全部 30 项活动版本匹配候选；收据为 homebrew/core、arm64、poured_from_bottle；QEMU SBOM 的根 bottle digest 为表内 `a399faaf…9d0d33` | 不声称重新校验了下载归档；三项复用仍沿用既有安装身份 |
| 相对安装前状态 | 新增 27 份收据，对应 23 个新增包和 4 个升级版本；原 40 份收据逐字未变且未删除；安装前采集的链接中 42 条发生变化 | 与预演范围一致；不是安装器全部动作的日志或完整恢复备份 |
| 加速器 / 机器 | `-accel help` 仅列出 `tcg`；`-machine help` 列出 `pc-q35-11.1`，`q35` 为它的别名 | 后续运行候选使用精确机器名；未实例化机器、未冻结全部 CPU / 设备参数 |
| 显示后端 | `-display help` 列出 `none`、`curses`、`cocoa`、`dbus` | 没有 SDL / GTK；本轮未打开 GUI 或验证显示交互 |
| 依赖与库检查 | `brew missing qemu` 退出 0；QEMU 30 项集合加相关既有工具共 38 包的 `brew linkage --test` 退出 0，无缺失库 | 既有工具含 OpenSSL 3、Git、ripgrep、leveldb、librime、Ruby、CocoaPods、FFmpeg；不等于各项目功能回归通过 |
| 实际加载路径 | 对 QEMU、qemu-img 与 OpenSSL 3 / 4 的 Homebrew 动态库递归读取 35 个 Mach-O 的依赖及 LC_RPATH，绝对库路径均存在，无待解析的 `@rpath` / `@loader_path` 引用；QEMU `--version` 的 dyld 加载日志确认 libssh 与 OpenSSL 4 加载 | macOS 系统库由系统加载器处理；没有用文件系统缺少 dyld shared-cache 实体误报缺库 |
| OpenSSL 默认与并存 | 本次 shell 的 `openssl` 解析到共享入口并输出 4.0.3；`opt/openssl@3/bin/openssl` 仍输出 3.6.4；共享 bin、include、lib、pkg-config 与 CMake 的 OpenSSL 路径均解析到 4.0.3 | 核对的是本次进程 PATH 与文件路径；未执行各项目构建，也未改变用户 shell 配置 |
| CA 引用 | OpenSSL 3 / 4 的 cert.pem 均指向 `/opt/homebrew/etc/ca-certificates/cert.pem`，文件非空，289562 字节；核验前后摘要一致 | 只记录文件状态，没有对比安装前 CA 内容或验证实际 HTTPS 信任结果 |

QEMU 与 qemu-img 实际链接 `opt/libssh/lib/libssh.4.dylib`，该库链接 `opt/openssl@4/lib/libcrypto.4.dylib`；这里 libssh 的 `.4` 是它自己的 ABI 名称，不是 OpenSSL 版本号。已有 FFmpeg 仍显式链接 OpenSSL 3 的 `libssl.3.dylib` / `libcrypto.3.dylib`，相关库存在且链接检查通过。QEMU 的 SBOM 包含构建时较旧 libssh 的记录，而安装收据与实际链接对应当前 libssh 0.12.2_1；不能把 SBOM 的构建依赖当作当前运行依赖清单。

固件使用本次 QEMU 包内的 `share/qemu/firmware/60-edk2-x86_64.json`：接口为 UEFI，映射为 flash / raw，目标为 x86_64、`pc-i440fx-*` / `pc-q35-*`；两个实体路径均位于当前 QEMU keg 内且存在。文件尺寸与 SHA-256 先由 Python 读取，再用系统 shasum 独立复核一致：

| 已安装文件（相对 QEMU keg） | 字节数 | SHA-256 |
| --- | --- | --- |
| share/qemu/firmware/60-edk2-x86_64.json | 770 | `455a4a2212f79f01c843bb1dc7d520862331f912fe37a33edbe1bff8c31de8fe` |
| share/qemu/edk2-x86_64-code.fd | 3653632 | `33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6dbb2a` |
| share/qemu/edk2-i386-vars.fd | 540672 | `5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e` |
| share/qemu/edk2-licenses.txt | 42903 | `1ddeaed2e7d2e9ecb960bdfc1b8ee45387aff70d056d985d145949af3951657c` |

固件代码与 VARS 均不同于第 1 节的 UTM 样本；VARS 尺寸也不同，不混用。后续每次 guest 运行从此模板复制独立 VARS，代码只读；本次没有写模板或做 UEFI 启动验证。已读取 QEMU 的 LICENSE / COPYING / COPYING.LIB 和 EDK II 汇总许可文本，登记见[许可与来源](../governance/licensing-and-sources.md)，不将 QEMU 许可代替固件许可。

本地证据位于忽略目录 `evidence/t01-qemu-20261019-8909c35/`，包括步骤脚本、退出码、收据、静态 / 实际加载路径、固件摘要和前后基线，连同任务缓存约 40 MiB。核验前后 67 份收据、9789 条采集链接、Homebrew commit 与 CA bundle 摘要完全一致。`brew linkage` 首次提示自动开启开发者模式；核对发现持久设置未写入，配置文件修改时间早于本轮。随后仅在检查进程设置 `HOMEBREW_DEVELOPER=1` 复验，配置哈希 / 修改时间不变且持久键仍缺省；没有执行全局模式切换。检查只使用已缓存元数据，没有下载软件或启动后台资源。

本节关闭 QEMU 版本、依赖缺失、默认 OpenSSL 与固件文件身份的核验缺口。真实 guest 启动 / 显示、网络 / 存储行为、RadishLex 打包及各项目功能兼容性仍未验证；T01、M0 与产品 AT 不能因此记为完成。

### 2.9 Rust nightly 安装后工具核验

2026-10-19，用户报告 Rust 安装完成，授权核对版本、组件、裸机 target 和链接器并更新文档。本轮只执行已装工具的查询与文件检查，没有运行安装器、下载发行归档、创建 Cargo 工程或编译源文件。工具链位于 `~/.rustup/toolchains/nightly-2026-10-13-aarch64-apple-darwin/`；下述路径除另注外均相对此目录。

| 核验项 | 实际结果 | 证据边界 |
| --- | --- | --- |
| rustc | `1.101.0-nightly (e9bbadf68 2026-10-12)`，完整 commit `e9bbadf6824e65b3583c2dda3a0926ec76df9143`；host 为 aarch64-apple-darwin，LLVM 23.1.3 | 版本查询通过，尚未编译项目 |
| Cargo | CLI 输出 `1.101.0-nightly (29c5daa1a 2026-10-07)` | manifest 的 Cargo 包版本为 `0.102.0-nightly`，与 CLI 展示版本分别记录；commit 一致，不把包版本误写成 CLI 版本 |
| rustfmt / Clippy | rustfmt `1.11.0-nightly`，Clippy `0.1.101`，均报告 e9bbadf682 / 2026-10-12 | 只验证工具启动，未对项目执行格式化或静态分析 |
| 七项组件 | rustc、cargo、宿主 rust-std、裸机 rust-std、rust-src、rustfmt、clippy，安装配置与候选集合完全一致 | manifest 中名称为 rustfmt-preview / clippy-preview；没有以 rustup 显示名差异误报缺失 |
| 裸机 target | installed 查询包含 `aarch64-apple-darwin` 和 `x86_64-unknown-none`；后者 lib 目录含 core / alloc / compiler_builtins 各一份 rlib 与 rmeta | 六文件存在且非空；三个归档内共 431 个 ELF 对象头为 ELF64、小端、x86_64、可重定位；不是实际链接或运行证明 |
| rust-src | `lib/rustlib/src/rust/library/` 中 core、alloc、std 的 src/lib.rs 存在且非空 | 文件身份记录于证据；目标使用预编译 core，T01 最小编译不必先启用 build-std；使用 alloc 仍需 allocator |
| 内置目标配置 | arch=x86_64、64 位、std=false、panic=abort、disable-redzone=true、code-model=kernel、linker=rust-lld、linker-flavor=gnu-lld | 来自已安装 rustc 的 target-spec-json；没有创建自定义 target 或修改目标配置 |
| 链接器 | `lib/rustlib/aarch64-apple-darwin/bin/rust-lld -flavor gnu --version` 输出 LLD 23.1.3，LLVM commit `aab11ee0ea5ea9a1d462ad0bc1fae9d9bb643019` | rustc / Cargo / rust-lld 均为原生 arm64 Mach-O；dyld 日志确认链接器加载同一 nightly 内的 libLLVM；未执行目标链接 |
| 默认与显式选择 | 1.96.0、1.97.1 与新增 nightly 并存；默认仍是 1.96.0，本次进程 RUSTUP_TOOLCHAIN=1.96.0；显式 +nightly 查询命中正确 sysroot | 本轮未改默认、全局 profile、目录 override 或 shell；后续 xtask 及其子命令须使用相同工具链选择 |

安装前保存的官方日期清单 SHA-256 为 `1919aa356b8e4a07d05fc702cd543c3d956d7caf5b3d80caa2a74e9c5d3b114c`。安装后的 multirust-channel-manifest.toml 不能与原文作全结构等同判断：本地版本缺少 artifacts / git_commit_hash 等字段，并补有空 components / extensions。逐项比较七项选定组件的日期、版本、available、gzip / xz URL 与摘要均相符；这是元数据身份核对，不是重新校验发行归档或全部已装文件来源。六个裸机库文件与 rust-lld 的实测 SHA-256 已经 Python 和系统 shasum 两路核对。

已读取随包 README 的许可说明并保留 COPYRIGHT.html、COPYRIGHT-library.html 与 licenses/ 文本，见[许可与来源](../governance/licensing-and-sources.md)。原工具链保留；核验前后 rustup settings 摘要、三套工具链目录清单及合计 35 份配置 / 组件元数据摘要一致，不扩大为安装前后全部文件逐字不变证明。

本地证据保存在忽略目录 `evidence/t01-rust-20261019-bac6c12/`，包括只读辅助脚本、14 项查询的命令 / 退出码、链接器实际加载输出、安装元数据、目标归档头检查、文件摘要与随包许可，约 16 MiB；不保存工具链二进制或目标库副本。没有后台资源。这关闭安装后工具核验缺口；最小内核实际编译、最终 ELF 链接、宿主 / 内核依赖隔离和产品运行仍未验证，T01、M0 与产品 AT 均未因此通过。

### 2.10 T01 工作区与真实链接验证

2026-10-25，用户授权按既有方案实施 T01、完成必要验证并同步文档，明确不提交或推送。本批使用已安装 nightly，不安装工具、引入第三方 crate、修改默认工具链或启动 guest。实际执行宿主为 aarch64-apple-darwin；Linux 仅同步了 CI 安装与验证入口，尚无运行证据。

workspace 包含 radishroot-kernel 与 xtask，版本继承根 workspace.package.version（26.10.0-dev），publish=false。Cargo.lock 由 `generate-lockfile --offline` 生成，实际 metadata / tree 确认两个包均无外部依赖和自定义 features。裸机入口 no_std / no_main、不分配堆；目标沿用预编译 core、kernel code model、无 red zone、panic=abort，项目 target flags 指定 static relocation 与非 PIE。build.rs 使用宿主标准库，不将 std 引入 guest。

| 验证 | 本批结果与边界 |
| --- | --- |
| 工具链选择 | 文件指定 nightly-2026-10-13；xtask 对照自身编译器 commit / host、rustup 项目文件来源、安装组件与目标，再用同一 rustup run 执行子命令；默认及进程外 RUSTUP_TOOLCHAIN 仍为 1.96.0 |
| 检查 / 宿主测试 | fmt、分别指定包 / target 的 check 与 clippy -D warnings 通过；13 项宿主测试覆盖参数、缺工具 / target、配置、ELF 正负例、子进程失败 / 信号 / 超时和输出冲突 / 符号链接 |
| dev 干净链接 | ELF 8,752 字节，SHA-256 `c4d1ececdf143cdbd108f951179daa1d7c6a997688fca063d78dfd655462ee26`；有效 RX LOAD 的文件 / 内存长度均为 6 字节，文件偏移 4096 |
| release 干净链接 | ELF 4,648 字节，SHA-256 `a57e47137d2e2f09e504797f3ea6073708d1836b082f21661050ffcae7e64bc8`；有效 RX LOAD 的文件 / 内存长度均为 4 字节，文件偏移 4096 |
| 独立复核 | 两种 profile 均为 ELF64 little-endian x86_64 ET_EXEC；Python struct 直接读取头 / program headers / symbol table，对照手写期望、LLD map 和系统 file；entry 与 _start 同为 0xffffffff80000000，LOAD 按 4 KiB 对齐，无动态段 / RWX；本例无 rodata / data，空段省略 |
| 实际入口拒绝 | 直接运行已编译 xtask 并传合成错误活动工具链 / RUSTFLAGS，分别返回 1 且未启动 Cargo 子命令；未知命令与非法超时返回 2；本地 CI install 拒绝返回 2。缺组件等列表情形使用独立合成输入测试，没有卸载用户工具 |
| 进程与输出 | 子进程独立进程组，默认 120 秒；超时返回 124、停止组并等待直接子进程。测试验证后代不能在超时后写标记；原始状态 / 信号及 PID 留在日志；独占输出冲突与符号链接拒绝 |
| 仓库 / CI | 本地治理检查及 14 项回归通过；两份根入口一致；CI 精确包含 repo-hygiene / rust-checks，聚合拒绝缺项、失败、取消、跳过及未知组件。实际 Ubuntu Actions 未执行，不借用旧治理运行作为 Rust 证据 |

本地复核调用了 CI 的 verify 辅助入口，它从 TOML 读取 channel 并依次调用相同的四个 xtask 命令；未调用 CI install。CI 使用 Python 3.11+ tomllib；安装仅允许在显式 GitHub Actions 步骤中进行，后续入口均锁定 / 离线且关闭自动安装。当前宿主工具面向 macOS / Linux Unix 进程组，不宣称 Windows 支持。

证据保存在忽略目录 `evidence/t01-workspace-20261025-e924ee1/`：独立 verify_elf.py、dev-elf.json / release-elf.json、entry-rejections.json、最终运行日志、产物及 map 副本、source-manifest.json 与 SHA256SUMS。最终四次入口的原始输出目录为 `target/t01-66377-1792904192915895000/`（check）、`target/t01-66665-1792904194387198000/`（test）、`target/t01-66825-1792904195976349000/`（dev）、`target/t01-67002-1792904196503277000/`（release）。候选为 e924ee1 加本批未提交文件的内容摘要，不将 HEAD 单独当作实现版本。

初次 Clippy 因两处手写取模对齐检查被拒绝，改用整数 is_multiple_of 后通过；独立复核脚本最初假设旧 Cargo 的 build 输出目录形式，读实际目录后改为在本次输出内定位唯一 kernel.map，未修改产物或放宽 ELF 断言。失败与最终成功日志分开保留。T01 的本地构建门槛已通过；高半区静态地址不证明分页、物理装载、栈或引导成立，M0 / AT01 / AT23 仍未通过。

### 2.11 T02 引导与失败验收

2026-10-25 接续 T01 提交 5b64eac，按明确授权引入 limine 0.6.5、uart_16550 0.9.0、bitflags 2.13.2，下载 Limine 12.9.4，并运行既有容器 / QEMU。nightly、全局工具链及 Homebrew 环境未变；依赖缓存隔离于 work/cargo-home。本节候选为该 HEAD 加未提交源码摘要，不将 T02 归入旧提交。

版本与摘要的可执行单一源是 [Cargo.lock](../../Cargo.lock)和 [tools/boot/manifest.tsv](../../tools/boot/manifest.tsv)。Limine 归档 SHA-256 为 `9880cc04681262214db9b824ed8a5c55296955027d8caa9f55ab52a6dd85d6a0`，BOOTX64.EFI 为 `8a7214bfd8b9a044b780f1e71b2752dc12ecc25c06b9cf39ac6a0a341c802282`；固定白名单校验 LICENSE / 3RDPARTY.md。许可及来源见[对应清单](../governance/licensing-and-sources.md#t02-已引入的引导组件)。

| 验证层 | 实际结果与边界 |
| --- | --- |
| 构建与宿主 | 16 项 xtask + 4 项 kernel lib 测试、fmt、宿主 / 裸机 check 与严格 clippy（含 boot-test）通过；本机 ci-rust.py verify 四入口通过。CI install 只补 `fetch --locked`，未运行远端 Ubuntu job |
| ELF | dev 为 1,624,056 字节、SHA-256 `10912198585e517eb8a1cee03c318b7bd61dbf9cab52eb961067d45d2feb59d1`；正常 release 为 26,824 字节、SHA-256 `5b7105d6a45e4f49030817a15662f0b0b6fef3615824b520ed1264c3e61c1b43`。独立 Python struct / symbol / map、file、shasum 验证 ET_EXEC、RX / R / RW 三段、无动态段 / RWX、入口在 text；T02 不再要求 _start 必须位于 text 首字节 |
| 工具与镜像 | 已有 Debian 镜像 ID 末尾 95b，完整身份由 manifest 校验；5 张专用 1 GiB GPT raw，ESP 128 MiB，DATA 剩余。QEMU 11.1.2 / pc-q35-11.1 / TCG / qemu64，1 vCPU / 512 MiB，VGA + modern VirtIO-blk，只读盘；沿用已核对的 EDK II code / 全新 VARS，无网络 / GUI |
| 正常启动 | 同一 release 镜像连续 10 次从新 VARS 进入内核；每次 32 段内存图、478,855,168 字节 usable、内核区域 kind=6、framebuffer kind=7，1024×768×32 / pitch 4096；reclaim=none。所有非 usable 类型保留，未建立分配器 |
| 负向启动 | 独立 boot-test release SHA-256 `81108f4267ae3a32a28d718941ffd22655c9d5756bb16d36ef4ee50cd72d3294`；4 张隔离盘分别注入协议不支持、缺必要响应、pitch=0、panic，得到 BOOT_PROTOCOL / BOOT_RESPONSE_MISSING / BOOT_FB_GEOMETRY / BOOT_PANIC，无 READY。注入复用生产校验路径，不是运行真实旧版 bootloader 或破坏任意指针映射 |
| 独立读盘 | Python 标准库读取主 / 备 GPT、CRC、布局、FAT mirrors / backup / FSInfo、8 个 ESP 文件与空 DATA；5 张盘全部通过。串口独立复核 10 正常 + 4 失败；正常盘启动前后哈希一致。最终 clippy 整理后重建的正常 / 故障 ELF 与 VM 候选逐字节相同 |
| 退出与资源 | 14 次 guest 都在终态后由宿主终止自身 QEMU 进程组并 wait；未发生超时，不把 signal 9 记成 guest 正常关机。容器 --rm、失败时按唯一名称清理；本批进程 / 容器已回收，大盘、缓存和证据留在忽略目录 |

成功套件为 `target/xtask-80390-1792905921433080000/`。最终静态检查 / 宿主测试 / dev / release 分别为 `target/xtask-82030-1792906031116898000/`、`target/xtask-82305-1792906033407006000/`、`target/xtask-82540-1792906035848953000/`、`target/xtask-82737-1792906036900443000/`；最终故障重建在 target/t02-final-fault-20261025。独立证据包为 `evidence/t02-boot-20261025-5b64eac/`，含独立读盘 / 串口 / ELF 脚本、结果、源码快照、日志和摘要；大盘不重复复制。

首轮真实启动确已进入内核，但固件 ANSI 输出与 ENTER 连行，严格宿主读取拒绝；修正内核先输出换行后才取得完整通过证据。开发链接最初因 .got orphan 拒绝，明确将必要 GOT 放在 RW 段；clippy 三项结构 / 表达检查修复后通过，未禁用检查。Docker 在沙盒中不可访问，获得同一范围的运行许可后执行；失败记录独立保留，不计算为通过。

这些证据关闭 T02 本批通过条件，并给 AT01 提供当前候选证据；T03–T05、M0、AT23 以及正常关机 / 写回 / 图形 / 安装仍无通过声明。硬件映射和响应对象生命周期仍依赖受信 Limine 交接，边界检查不声称安全解析恶意 bootloader 任意指针。

### 2.12 T03 内存、PCI 与 DMA 边界验收

2026-10-25，接续已提交的 T02（63e2193），按明确授权完成本地 T03。固定工具、Limine、固件与 QEMU 参数沿用第 2.11 节；仅新增已批准的 19 个 crate 和相应 lockfile / 许可文本，没有更新已有依赖、全局工具链或宿主设置。精确集合与功能开关见 [MVP T03](../mvp-plan.md#t03-本批实施契约)，来源见[许可登记](../governance/licensing-and-sources.md#t03-已引入的内存与设备组件)。隔离缓存仍在 work/cargo-home；xtask 没有新增外部运行依赖。

| 核对项 | 实际结果 |
| --- | --- |
| 物理页 / 堆 | 正常候选内存图 34 段、usable=478699520；独立按整页、低 1 MiB 和 512 MiB 上界核算可分配 116710 页。1 MiB 堆物理起点 0x100000；两页测试起点 0x200000、16 KiB 对齐；实际写入 / 读回、释放后计数通过 |
| PCI / MMIO | 固定 00:02.0、1af4:1042；6 个 capability；BAR1=0x81011000 / 0x1000，BAR4=0xc000000000 / 0x4000。UC / NX 新映射、真实寄存器读取通过，features=0x10130006e74、queue_size=256；bus_master=0，未创建队列 |
| HAL / 隔离 | 两页清零分配、512 字节 Both bounce、清空原缓冲后 CPU copy-back、未提交取消和精确释放通过。超时状态注入同时拒绝 unshare 和原始 dma_dealloc，保留原缓冲及 bounce 到 halt；不是设备实际 DMA 或超时实验 |
| 正常 / 失败 | 10 次正常启动均达到 T03 READY；4 类 T02 故障回归通过；t03-invalid-range / t03-bad-capability / t03-dma-timeout 分别得到 MEM_RANGE / PCI_CAPABILITY / DMA_IN_FLIGHT，均无 T03 READY |
| 本地 Rust | fmt、宿主 / 裸机 check、严格 clippy、17 项 xtask 与 11 项 kernel lib 测试、dev / release 全新链接均通过；本机 ci-rust.py verify 成功，远端 Ubuntu Actions 未运行 |
| 独立 ELF | dev 4216968 字节，SHA-256 `1e63e1662b2de88689f7f608ca786256c0ff12764ac47193fac19627d56b5b7e`；release 115168 字节，SHA-256 `26b721a90cc01129a7fc28a09854c662ba5b6d4042e3b19122c4072cde20587b`。header、符号、LLD map、file / shasum 一致；静态 RX / R / RW，无 RWX 或动态段；独立构建 release 与 VM 候选相同 |
| 独立磁盘 | 8 张 1 GiB 稀疏盘，每盘主备 GPT CRC、分区范围、FAT、28 个 ESP 文件与空 DATA 通过；包括 23 份完整 crate 许可文本。正常盘前后 SHA-256 `16564880546ade65958277d223b8e4492510cd9f0783a45c49584155db023e53` 相同 |
| 资源 | 成功套件实际分配 6422528 字节；每次全新 VARS、只读盘、1 vCPU / 512 MiB、60 秒上限；约 95 秒完成全套。本次记录的 QEMU PID / 容器名无残留，宿主终止与 wait 不代表 guest 正常关机 |

完整 VM 运行在 `target/xtask-4531-1792908412840199000/`；check / host-test / dev / release 分别为 target 下的 xtask-2119-1792907937346940000、xtask-2649-1792907943234116000、xtask-2889-1792907945368498000、xtask-3337-1792907952139744000。初次单次运行及早期编译失败另行保留。独立证据在 `evidence/t03-platform-20261025-63e2193/`，含源码内容清单、日志 / ELF 副本、GPT / FAT / 串口 / ELF 复核脚本、清理记录和 SHA256SUMS；精确包元数据与许可复制摘要在 `evidence/t03-preflight-20261025-63e2193/`。候选仍含未提交源码，HEAD 不能单独代表它。

T03 只在固定四级分页、单 BSP / IF=0、root bus 0 下验收；映射与页表生命周期依赖受信 Limine。上游 PciTransport Drop 有无界复位循环，当前以 ManuallyDrop 和 100000 次状态轮询控制停止，保留映射 / claim 到 halt；真实队列与设备 DMA 的完成、超时、复位和释放仍是 T04 的必要工作。AT03 / AT23 / M0、文件系统、写回、正常电源流程、GUI、安装与真机均未通过。

### 2.13 T04 块存储与 GPT 验收

2026-10-25，先提交 T03 为 a37b3ef，再按明确授权实现本地 T04。新增 8 个精确 GPT 依赖，共 30 个 registry 包；既有版本 / checksum 不变，gpt_disk_io 无 std / alloc，xtask 仍只用 std。用途、特性与许可见 [MVP T04](../mvp-plan.md#t04-本批实施契约)及[许可登记](../governance/licensing-and-sources.md#t04-已引入的-gpt-组件)。无新的工具安装、全局修改或外部发布。

| 核对项 | 实际结果 |
| --- | --- |
| 块设备 | 固定唯一 modern VirtIO-blk、512 字节块、2097152 扇区、16 项 queue、FLUSH 协商；真实 used token / status 验证，单 outstanding 请求；写入拒绝与容量检查在提交前完成 |
| GPT / 分区 | MBR、主备 header / array CRC、互指、范围、GUID 与重叠校验；DATA 为 264192 + 1832927 扇区，ESP 为 2048 + 262144 扇区；完整一致才允许写，单份格式损坏只读降级，两份损坏或冲突拒绝，不自动修复 |
| 写入 / flush / 读回 | 专用 DATA 尾部 4096 字节，以 0xcc 初始化；1024 字节对齐写后，在偏移 1023 处写 1000 字节。实际 flush 成功、30 次请求；guest 读回 checksum=0x52c1276bd54e4b2b。宿主比较整盘精确期望，重启同盘只读 13 次请求再验证；ESP / GPT / 非目标字节不变 |
| 失败 / 资源 | 越界 6 项与只读拒绝未提交 IO；真实 unsupported 响应可识别。超时实际提交请求后隐藏 used 观察，保留 queue / bounce / 原缓冲，拒绝释放与复用后 halt；正常路径读回 status=0、关闭 bus master 后释放，allocations=0 / shared=0。缺 FLUSH 用例为明确能力拒绝注入 |
| 本地 Rust | fmt、宿主 / 裸机 check、严格 clippy、19 项 xtask + 20 项 kernel lib 测试、dev / release 全新链接通过；本机 ci-rust.py verify 成功，Ubuntu Actions 未运行 |
| 独立 ELF | dev 5081112 字节，SHA-256 `256956d936fe271ea2f3f06a9992b649cd33f8d143e81ef78758a713482ee68b`；release 177792 字节，SHA-256 `e4146371500b07838e9a8d97ff50a3f94809f014f187b5b54a15c472c8230819`。独立 header / symbol / map / file 核对静态 RX / R / RW，无动态或 RWX；两个 VM 分组内核与最终构建一致 |
| 独立磁盘 | Python 标准库独立复核 16 张引导 / 源盘的主备 GPT、FAT、每盘 36 个 ESP 文件（31 份 crate 许可），以及 4 张数据副本的全盘字节；不导入生产 guest / xtask 代码。GPT 故障 patch 由独立 zlib CRC 计算，所有非目标字节不变 |
| 覆盖组成 | 10 次正常 + 7 个既有故障取自第二次完整运行，随后该套件在 GPT 降级请求计数检查处失败；修复宿主验收器并补回归后，7 个 T04 用例通过 storage-only 定向复跑。内核没有因该修复改动，ELF 字节一致；不声称最终完整 test --vm 单次返回成功 |
| 资源 | 累计 36 盘 / 46 次启动尝试，盘逻辑容量均为 1 GiB、实际分配共 77512704 字节；用户已扩额至 40 盘 / 60 次。任务 PID / 容器回收，VARS / 日志 / 盘保留；不触及物理盘、用户资源或全局设置 |

只读引导盘由固件经 virtio-scsi / scsi-hd 读取，内核仅操作唯一 VirtIO-blk 数据盘。最初使用 ide-hd 被 QEMU 明确拒绝只读 backend，在进入 guest 前退出；修复连接方式后取得真实写入与重开证据。随后验收器把所有正向路径统一要求至少 6 次请求，错误拒绝了主 header CRC 无效而跳过主数组的 5 次请求路径；现在按 probe=6、write=30、reopen=13、primary-gpt=5 精确检查，回归同时拒绝多余请求与未释放分配，没有放宽设备停止 / flush 条件。

最终宿主检查、测试、dev / release 构建分别在 target/xtask-71032-1792910548197340000、xtask-71597-1792910554435240000、xtask-71920-1792910558076073000、xtask-72464-1792910563116662000。普通与既有故障分组在 `target/xtask-50967-1792910327737635000/`；T04 定向成功在 `target/xtask-69790-1792910523904326000/`；首次探测和只读 backend 失败另存于 xtask-44040-1792909866261728000 / xtask-48015-1792910071125534000。独立脚本、源码内容摘要、日志副本、ELF、清理报告与 SHA256SUMS 在 `evidence/t04-storage-20261025-a37b3ef/`，依赖原始身份在 `evidence/t04-preflight-20261025-a37b3ef/`。

T04 只验证固定平台的块存储闭环；轮询为次数预算，不宣称真实时钟超时。超时保留到 halt，不等于设备已停止 DMA；缺 flush 与超时注入分别注明机制。宿主终止后重开同一盘不代表正常电源循环，DATA 块模式测试不代表 FAT 文件语义或文件系统断电一致性。T05、AT03 / AT23 / M0、GUI、安装、其他平台仍未通过。验收时 T04 源码尚未提交，原候选身份由 a37b3ef 与源码摘要共同确定；随后按用户要求提交，原始证据保留，不改写历史快照。

### 2.14 T05 FAT 持久化与独立读盘验收

2026-10-25，基于 dev / 4bacb57，工作区初始干净、领先本地 origin/dev 4 个提交。按本轮明确授权增加 FAT 候选并执行专用 VM 验证；未提交、fetch、push、PR 或发布。用户重新批准了 T05 依赖与资源，不沿用 T04 的运行额度；原有测试盘、失败材料和依赖版本保留。

| 冻结项 | 本批验证的组合 |
| --- | --- |
| 编译与引导 | nightly-2026-10-13、x86_64-unknown-none；Limine 12.9.4 / limine 0.6.5 / protocol revision 6；固件哈希仍以 tools/boot/manifest.tsv 为准 |
| 设备与 GPT | virtio-drivers 0.13.0、modern PCI、16 项队列、512 字节扇区、真实 FLUSH；gpt_disk_io 0.17.0，无 std / alloc；沿用 GPT 主备与 DATA 边界校验 |
| FAT | fatfs 0.4.0，Git commit `38b3a7950bf0e765e34286d1dfe76c519ee347e2`；仅 alloc / lfn / unicode，无 std / chrono。30 个既有 registry 包逐项不变，仅新增 1 个 Git 包；MIT 文本随 ESP 保存 |
| 镜像与宿主 | 1 GiB raw / GPT；ESP LBA 2048–264191，DATA LBA 264192–2097118；FAT32 / 512 字节，DATA 每簇 4096 字节；QEMU 11.1.2、pc-q35-11.1、TCG、qemu64、1 vCPU / 512 MiB、cache=writeback、aio=threads、无 snapshot |
| 独立工具 | manifest 固定 Debian 镜像；mtools 4.0.48-1、dosfstools 4.2-1.2。guest 退出后以只读 bind 读盘；mdir / mcopy 比较名称、长度、原始字节；分区副本 fsck.fat -n；Python 标准库另核对 GPT、FAT、ELF 和合成期望 |

| 验证层 / 案例 | 实际证据 |
| --- | --- |
| 宿主与裸机 | fmt、宿主 / 裸机 check、严格 clippy（含 boot-test）、21 项 xtask + 28 项 kernel lib 测试通过；dev / release 全新链接及独立 ELF 复核通过。没有运行本轮远端 Actions，也没有重跑本机 ci-rust.py 聚合入口 |
| 正常 FAT 写入 | 读取预置 seed.txt（23 字节），新增 11 文件共 1055220 字节；跨扇区二进制 4099 字节、UTF-8 内容 2425 字节、one-mib.bin 1048576 字节。全部文件及目录元数据显式写回、卸载及最终设备 flush 成功；49628 次流操作、27147 次块请求、111 次成功 flush；无首错或被阻止的后续调用 |
| 同盘重开 | 使用同一 DATA 副本，新的 VARS、只读 backend 重新启动；12 文件精确读回，5474 次块请求、0 写入字节 / 0 flush；整盘 SHA-256 与上次写入后相同。两个时点的独立 mtools 提取与 fsck 均通过 |
| 只读与坏 FAT | 创建新文件在 DATA 入口以 BLOCK_READ_ONLY 拒绝，首错保留 Create 阶段，无设备写入 / flush；原盘摘要不变。主备 BPB 的 bytes_per_sector 清零后返回 FAT_CORRUPT，不修复；整盘摘要不变，独立 Python 同样拒绝几何参数 |
| 文件目录项写回失败 | 1649 个内容字节已确认写入后，在 FileSync 的下一次流写入注入失败；锁存原始阶段，阻止 3 次后续调用，无 READY。mtools 读到 failed.txt 长度为 0；fsck -n 记录一个未引用簇及脏标记，退出 1，明确未修改磁盘 |
| 卷 FSInfo 写回失败 | 11 文件内容及目录项写完后，在 VolumeSync / offset=512 注入写失败；阻止后续析构写回，无 READY。独立文件内容可读且相符，但 fsck 报告脏标记和过期空闲簇统计（228661 / 实际 228393），退出 1；保留失败结论 |
| 最终设备 flush 失败 | 完成卷写回及卸载后将最后一次请求类型改为 unsupported，取得真实负响应，阶段保留 Flush / DeviceFlush，无 READY；先前已有 110 次 flush 不能替代最后一次。独立文件和 fsck 均可通过，仍报告持久化未确认 |
| 既有行为 | 7 个 T04 存储用例完整分组回归通过；另一次默认构建只读启动通过，默认入口未写 FAT。所有正常返回均确认设备复位、bus master 关闭、allocations=0 / shared=0；T04 超时用例继续隔离并 halt |
| 独立结构与边界 | 17 张引导 / 源盘的 GPT、FAT、每盘 37 个 ESP 文件通过（含 32 份 crate 许可）；正常 DATA 的 12 文件经独立 Python 再验证。6 张 T05 DATA 副本所有分区外字节与源盘相同；原有 59 张测试盘均存在 |

名称实测冻结：每分量 1–255 个 UTF-8 字节、BMP 字符；255 个 ASCII 或 85 个三字节汉字可创建，256 个 ASCII / 86 个汉字及补充平面字符拒绝。中文、空格与大小写保留；ReadMe / readme、Straße / STRASSE、长名短别名、已有文件及目录共 5 项碰撞拒绝，8 项非法 / 超长路径拒绝。`é.txt` 与 `e` + combining acute 名称不同且内容分别保留，不做 Unicode 规范化。同目录和跨目录文件重命名通过；库对仅大小写重命名返回 no-op，不承诺改名。跨卷、跨目录目录迁移、覆盖原子性、中文输入 / 字形显示均不在本次结论内。时间戳仅为库默认 1980-01-01，不代表真实创建时间。

最终 dev ELF 为 7022952 字节，SHA-256 `d23dd126f3837ac85e4f9c7b93b78dca699ab7ad745bda13cc43e364085d17b2`；release 为 178992 字节，SHA-256 `e227730071d636cd247e84a08fbc5e3aabb62d364988a655e24f38c8808d037b`。独立 header / symbol / LLD map / file 一致，静态 RX / R / RW，无动态段或 RWX。两个 VM 分组所有 boot-test 内核的 SHA-256 均为 `7eca17f7a30b6855ef2ad5f3ac6c40242c5541fa190656f3fb867d0c12d71440`；普通启动与独立 release 构建逐字节相同。

T05 成功分组位于 `target/xtask-92988-1792912024474624000/`；T04 回归为 `target/xtask-97592-1792912230948893000/`；默认启动为 `target/xtask-27641-1792912462909417000/`。最终 check / host-test 为 target 下 xtask-31926-1792912933128723000-0 / xtask-31927-1792912933129705000-0；dev / release 构建为 xtask-26477-1792912386451718000 / xtask-26976-1792912426985590000。证据入口为 `evidence/t05-fat-20261025-4bacb57/`，依赖身份为 `evidence/t05-preflight-20261025-4bacb57/`。证据包含源码清单、日志、ELF、独立读取脚本及结果、文件摘要、资源检查和 SHA256SUMS；大盘仍保留在原 target 路径。

资源实际使用：27 张 1 GiB raw / 15 次启动；raw 实际分配 93159424 字节，包含保留的 FAT 分区材料合计 203735040 字节（约 194.30 MiB），低于本次 32 盘 / 32 次 / 1 GiB 实际分配上限。每次启动均在 60 秒内观察终态并回收；容器无网络、1 CPU / 256 MiB / 64 PID。本任务 PID 与容器名经只读查询确认无残留，不清理新旧磁盘、既有 Docker 服务或其他用户资源。

首次 clippy 报告非 Drop 类型的显式 drop，改用借用自然结束后通过；两次沙盒 VM 入口均在 Docker 查询处因权限被拒绝，没有启动 guest 或生成磁盘，提升同目标权限后分组通过；这些失败日志保留。最终宿主复验在 xtask-28821-1792912707628335000 暴露了并行测试的同时间戳目录碰撞；为目录名增加原子序号，补同时间戳 16 并发回归后通过。VM 验收后仅修正宿主目录命名及回归测试，内核 / 已验收 ELF 未改变。没有放宽存储断言、删除故障覆盖或将环境拒绝当作产品通过。

**结论边界：**T05 本地门槛通过、AT02 在固定同盘重开方式下取得证据；不是完整 test --vm 单次运行，也不替代正常电源流程。M0、AT03 / AT23、T06 及后续图形 / 安装未通过；本批不进入 T06，不提交或推送。FAT 失败可留下部分内容和不可达簇，未提供断电事务保证。候选身份是 4bacb57 加源码内容清单，不能仅以 HEAD 代表未提交实现。

### 2.15 M0 收口证据核对

2026-10-25，按用户要求汇总 T01–T05 与关联 AT，只读核对既有证据并提出补验方案。候选为 dev / `0e5600e6f923b3e3c30a3ce7975987650f235880`；开始时工作区干净、暂存为空，领先本地 origin/dev 5 个提交，未刷新远端。此次没有运行构建、宿主 Rust 测试、VM 或容器，没有创建测试盘，也没有进入 T06。

**完整性与候选对应：**五份证据包的 SHA256SUMS 分别为 211、468、652、929、1061 条，共 3321 条，全部匹配。T05 的 118 项源码清单逐项对照 `0e5600e`，只有 current 与月度日志的提交状态文字变化，生产代码、配置、依赖与验收器一致。证据清单校验不等于重放测试，也不涵盖未列入清单的大盘；本轮另用独立 Python 期望只读比较最新 T04 回归的四张 DATA 盘全部字节，均通过。脚本和机读结果在 `evidence/m0-audit-20261025-0e5600e/`，历史包未修改。

| 任务 | 已通过证据 | 当前候选的覆盖与缺项 |
| --- | --- | --- |
| T01 | 两种 profile 全新链接、独立 ELF、工具链 / 依赖隔离与宿主失败检查；原始包 t01-workspace-20261025-e924ee1 | T05 的 49 项 Rust 测试及 dev / release 构建继承并扩展这些检查；尚缺从干净导出源码重放的收口记录 |
| T02 | 原始包 t02-boot-20261025-5b64eac 中同镜像十次全新 VARS 启动及四类故障 | T05 候选只有一次默认只读启动；旧候选的十次启动及故障不能直接当作当前候选复验 |
| T03 | 原始包 t03-platform-20261025-63e2193 中内存 / PCI / HAL、十次启动及三类注入；当前宿主测试覆盖分配与 DMA 状态机 | T05 正常路径和 T04 真实 DMA 回归通过；三类独立 T03 故障未在当前候选重跑 |
| T04 | 原始包 t04-storage-20261025-a37b3ef；T05 包中七个存储用例回归、同盘读回与设备停止 / 隔离记录 | 当前七用例通过；本轮额外独立比较四张 DATA 盘，目标内容及所有非目标字节相符 |
| T05 | t05-fat-20261025-4bacb57 中七用例、12 文件独立比对、mtools / fsck、名称与写回失败证据 | 当前文件持久化可行性通过；正常电源流程和正式文件服务分别属于 T12、T08 |

表中历史包均在 evidence/ 下；原始执行目录、配置、哈希及失败说明见第 2.10–2.14 节。历史阶段通过与最新候选补验是两个状态，不能因已有历史成功省略受影响回归。

| 验收 | 本次结论 | 依据与边界 |
| --- | --- | --- |
| AT01 / FR01、FR12 | 部分覆盖 | 当前默认启动保留内存图与 framebuffer 事实；缺同一当前镜像连续十次全新 VARS 启动和四类 T02 故障回归 |
| AT02 / FR02、FR11、FR12 | 通过，限固定 M0 平台 | 预置 seed、跨扇区 / UTF-8 文件、完整 flush、同盘只读重开及退出后独立工具路径 / 长度 / 字节一致；cache=writeback、aio=threads、snapshot=off。宿主终止后重开，不是 guest 正常 reboot / shutdown |
| AT03 / FR04、FR11 | 通过，限固定 M0 平台 | 宿主覆盖块范围 / 溢出 / 对齐；真实 guest 六项分区边界与只读拒绝均 submitted=0，未对齐读改写和独立全盘期望一致；正常路径确认 reset / bus master=0 后释放；真实请求发布后的超时注入保留 3 shares / 2 allocations，release_rejected=1、allocation_rejected=1、reused=0，代码在原借用栈内 halt |
| AT23 / FR12 | M0 范围部分覆盖；完整项留 M5 | 锁定工具、离线命令、超时 / 失败码、产物与源码关联、许可来源、独立校验及验收器坏结果拒绝测试已有证据；缺干净源码目录重放当前构建 / 测试与最新候选 VM 收口记录。demo / 安装尚未实现，不计通过；远端 Linux CI 未运行 |
| M0 | 部分覆盖，尚未收口 | 真实存储闭环和固定候选组合已有证据；待 AT01、AT23 的 M0 缺项及 T03 独立故障回归补齐 |

AT03 的超时实验隐藏 used 完成观察，真实请求已提交；它证明可能仍有 DMA 时不释放缓冲区，不声称设备一定仍在 DMA 或已停止。此结论来自本轮对正式 AT03 条款、当前日志、独立字节及所有权代码的复核，更新第 2.14 节结束时尚未统一验收的状态，不改写历史记录。T12 前置为 T06 / T08 / T10，正常电源验收不作为 M0 的额外前置；AT05 / AT14 仍未执行。

**必要补验方案（本轮未执行）：**

1. 从 `0e5600e` 导出独立干净源码目录，核对跟踪文件清单；只复用已授权且哈希匹配的 Cargo 缓存、Limine 资产及工具镜像。运行既有 `check`、`test --host`、dev / release `--fresh`，保留实际命令 / 退出码 / ELF / 源码摘要。重放既有坏串口、提前释放及错误成功拒绝测试，并对独立文件比较的副本改动一个字节，记录原件通过、坏副本拒绝；不修改生产断言或历史材料。缺缓存或身份漂移立即停止，不下载依赖。
2. 必须补齐的 VM 覆盖是十次正常启动、四类 T02 和三类 T03 故障，共 17 次。当前统一入口没有只跑这些组的选项；推荐直接从该干净目录执行已有 `test --vm` 一次，连同已通过的 T04 / T05 取得最终验收器下的连续编排证据。它共 31 次启动、22 张引导 / 源盘加 10 张 DATA 副本；这是现有入口的执行方案，不把“一次全套成功”新增为所有阶段的普遍硬门槛，也不为本轮另造测试入口。
3. 新盘独立验证 GPT / FAT、文件路径 / 长度 / 字节、DATA 外边界、正常 / boot-test ELF 与构建候选关系；在每个 guest 退出后读盘，保留错误注入的脏卷与部分修改事实。归档来源清单、哈希、步骤与资源清理报告；若源代码发生变化，重新界定候选及受影响检查，不能继续拼接旧结果。

建议资源申请为**新增最多 32 张 1 GiB 稀疏 raw、31 次 QEMU 启动、盘及分区材料实际分配合计不超过 1 GiB**，不包含失败后额外重试；源码 / 构建 / 日志另预留最多 2 GiB。固定 QEMU 11.1.2 / pc-q35-11.1 / TCG / qemu64，串行 1 vCPU / 512 MiB；复用既有 Debian 工具容器，无网络和 GUI。预计 10–15 分钟，guest 每次 60 秒上限，构建 / 工具命令按既有 120 秒子进程上限；45 分钟总预算或资源上限触及即停止并保留失败，另报差异。结束回收本次 QEMU 进程组及临时容器，源码导出、盘、VARS、日志和证据保留在任务专用忽略目录，不清理旧资源。用户要求新增 VM 资源先确认，此段仅为具体方案，不是已获运行授权。

固定候选继续使用第 2.14 节的工具、依赖、镜像布局、flush 与名称语义；本轮不变更依赖、格式或兼容范围。原创许可证选择与发行材料审查仍待后续处理，本次结论限本地可行性验证，不宣称分发就绪；Linux Actions、GUI、安装、正常电源和真机证据均不能由本轮替代。

### 2.16 M0 干净候选补验与本地收口

2026-10-25，用户明确要求提交工作区更改并按第 2.15 节额度继续。先提交四份核对文档为 `87d099d docs(m0): 汇总阶段证据与收口补验方案`，未推送；该提交相对 `0e5600e` 只有文档变化。随后使用本地独立克隆 `/private/tmp/radishroot-m0-replay-20261025-87d099d/source` 验证完整候选 `87d099d2140aafe2c44c21d588f96868e4d37a7b`，保留原仓库和全部旧盘。独立目录避开原仓库的上层 Cargo 配置；没有创建产品分支、改动产品代码、更新依赖或进入 T06。

**干净输入：**118 个正式文件逐项对照候选 Git blob 及执行位，开始时没有 target/，运行前后跟踪文件均无变化。仅复制已校验的三项 Limine 资产，复用现有 Cargo 缓存与固定 Debian 工具镜像；30 份 registry 归档摘要符合 Cargo.lock，解包文件逐字节符合归档，fatfs 跟踪源码符合固定 Git commit。构建均使用锁定 / 离线入口，未下载工具或依赖，未修改全局配置。

| 补验层 | 实际结果 |
| --- | --- |
| 本地 Rust / T01 | 干净目录的 check、test --host、dev / release --fresh 四入口成功；fmt、宿主 / 裸机检查和严格 clippy、21 项 xtask + 28 项 kernel lib 测试通过。没有额外重跑 ci-rust.py 聚合入口；这里记录的是其四个实际子入口 |
| 独立 ELF | dev 7022984 字节，SHA-256 `63173ea70afc896f7ba557621ed5e32f3658cfd5abffbd56ce458bf98d1f51b5`；release 178992 字节，SHA-256 `e227730071d636cd247e84a08fbc5e3aabb62d364988a655e24f38c8808d037b`。Python header / symbol / map 与 file / shasum 一致；ELF64 x86_64 ET_EXEC、RX / R / RW、无动态段 / RWX。release 与 T05 相同；dev 按本次独立身份记录，不承诺位级可复现 |
| AT01 / T02 | 同一正常 release 镜像连续十次启动，均从独立新 VARS 进入内核；内存图、framebuffer 与保留区事实独立核算通过。四类 T02 故障得到预期串口原因并停止，无成功终态 |
| T03 回归 | 三类独立故障 MEM_RANGE、PCI_CAPABILITY、DMA_IN_FLIGHT 全部通过；正常路径页数、可用区、内存读写、PCI / BAR 与 HAL 状态复验通过 |
| AT03 / T04 | 七用例通过；边界 / 只读拒绝 submitted=0，跨扇区读改写及同盘读回一致。独立 Python 逐字节核对四张 DATA 副本，目标内容与 GPT 注入符合独立期望，所有其他字节不变；正常停止后释放，超时仍保留 3 shares / 2 allocations、拒绝释放与复用 |
| AT02 / T05 | 七用例通过；写入 11 文件共 1055220 字节，含预置文件在内共 12 文件由 guest、mtools 及独立 Python 比较路径 / 长度 / 原始字节。正常写入与同盘重开摘要一致，最终设备 flush 有真实完成证据；六张 DATA 副本分区外字节不变 |
| 写回失败 | 文件目录项 / FSInfo / 最终设备 flush 故障均保留失败与首错；两项脏卷的 fsck -n 返回 1，其他五次独立检查返回 0。最后设备 flush 失败时即使文件可读仍为 persistence=unconfirmed，不改记成功 |
| 整套编排 | 现有 test --vm 一次返回 0：10 普通 + 4 T02 + 3 T03 + 7 T04 + 7 T05，共 31 次，实际约 278 秒，没有 VM 失败重试。全部故障镜像使用同一 boot-test ELF，SHA-256 `7eca17f7a30b6855ef2ad5f3ac6c40242c5541fa190656f3fb867d0c12d71440`，与 T05 相同 |
| 独立镜像 / 配置 | 22 张引导 / 源盘逐盘核对主备 GPT、FAT、每盘 37 个 ESP 文件及整盘摘要；31 组实际 QEMU 参数符合固定平台、cache=writeback / aio=threads / snapshot=off、无网络 / GUI。十次普通启动使用同一只读镜像，每次复制已核对的固件 VARS 模板到新目录 |
| AT23 的 M0 范围 | 干净源码、固定缓存、构建 / 测试重放、候选 / 产物 / 来源关联、独立复核通过。既有坏串口 / 提前释放 / 错误成功拒绝测试通过；另复制实际提取的 4099 字节文件，在偏移 1023 改一字节，独立比较原件返回 0、坏副本返回 1，原件保持不变 |
| 资源与退出 | 新增 32 张 1 GiB raw，逻辑合计 32 GiB，raw 实际分配 98582528 字节；51 份稀疏分区材料实际分配 134840320 字节，两者合计 233422848 字节，低于 1 GiB。源码 / 构建 / 日志及归档时证据约 686067712 字节，低于 2 GiB；运行器监测数量、空间和 45 分钟总预算，归档时累计约 680 秒。309 个记录 PID、35 个唯一容器名均无残留，旧 86 张盘保留 |

实际执行目录均在上述独立克隆的 target/ 下：check 为 `xtask-55593-1792922508097693000-0`，host-test 为 `xtask-56183-1792922515081428000-0`，dev 为 `xtask-56496-1792922519263144000-0`，release 为 `xtask-56963-1792922525534930000-0`，完整 VM 为 `xtask-57713-1792922573231615000-0`。小型源码 / 日志 / ELF / 独立脚本 / 机读报告及 SHA256SUMS 归档在 `evidence/m0-replay-20261025-87d099d/`；大盘、VARS、完整构建与分区材料保留在独立克隆中，没有重复复制或删除。

环境拒绝分别保留：提交时沙盒拒绝 index.lock、镜像预检拒绝 Docker socket、退出核对拒绝 ps，均取得同范围最小权限后完成。临时目录中的 shell 默认 Python 是 `/usr/bin/python3` 3.9.6，仓库检查在入口明确拒绝；改用已安装的 `/Users/luobo/.local/share/mise/installs/python/latest/bin/python3` 3.14.5 后，118 文件治理检查通过。没有修改检查器、安装 Python 或把环境拒绝记成产品通过。

**结论：**固定平台的 AT01、AT02、AT03 与 AT23 的 M0 已实现范围通过，T01–T05 依赖组合、真实存储及干净候选重放满足 M0 本地退出门槛。完整 AT23 留 M5；未运行远端 Linux Actions，未实现 / 验收正常电源、GUI、安装、真机或发行许可，不以本地收口代替这些证据。第 2.15 节缺项由本节关闭，历史报告保留原状态；本批结束于 M0，不进入 T06。

### 2.17 T06 异常、时钟与事件循环验收

2026-10-25，用户要求先提交 T06 方案，再按方案实现，明确批准 x86_64 中断 ABI 特性及 48 次启动 / 51 张新盘、盘及分区材料 1.5 GiB、其他材料 2 GiB 的额度，新增其他依赖或超额先确认，暂不推送。先提交 `5a5275818a7cce3ab87d02e24dcc454084d6353c docs(t06): 明确运行时实施契约与验证方案`，随后实现。运行候选由此提交加 `candidate-manifest.json` 的 124 个文件内容共同确定，不能只用 HEAD 代表尚未提交的实现；验收后仅更新正式文档及 Serial 的安全前提注释，剔除该注释后运行时代码与候选一致，未改变可执行逻辑。

**固定实现：**沿用 QEMU 11.1.2 / pc-q35-11.1 / TCG / qemu64、1 vCPU / 512 MiB、无 GUI / 网络及既有 Debian 工具镜像。只为既有 x86_64 0.15.5 开启 abi_x86_interrupt；nightly-2026-10-13、Cargo.lock 与包版本不变。30 个 registry 归档及解包源码、fatfs 固定 Git 源码再次独立核对，无新包、下载或安装。

内核先安装早期异常 IDT，再经 T03 内存初始化建立 HPET、GDT / TSS / 最终 IDT；double fault 使用独立 16 KiB 静态 IST 和 4 KiB guard。HPET 由有界 ACPI RSDP / XSDT / HPET 表校验后通过共享 MMIO 映射器建立 UC 映射；实测 64 位主计数器 period=10000000 fs。PIC 重映射、仅开放 IRQ0，PIT divisor=11932 负责约 100 Hz 唤醒；不启用 HPET comparator / legacy replacement，显式关闭并读回本地 APIC enable 位。IRQ 只更新计数 / pending 和 EOI；主循环消费合并唤醒与 64 项 FIFO，每轮普通事件上限 16。帧 / 堆分别报告 total / tracked / allocated 与未知范围，不把固定堆重复计入帧总数。

默认只读存储探测后进入持续循环；100 次实际唤醒后发布 T06 READY，宿主不在 T04 READY 提前结束。请求采用 2 秒时限、复位 100 毫秒，同时保留次数保险并分别报告 Deadline / PollBudgetExhausted / Clock；已提交请求失败仍原地保留 DMA 与原始借用缓冲。致命诊断不取普通锁、不分配、不 unwind，总串口字节及每字节发送轮询有界。T02–T05 独立用例仍保留各自终态。

| 验证层 | 本轮事实 |
| --- | --- |
| 宿主规则 | `target/xtask-83738-1792926487569177000-0` 的 test --host 通过 24 项 xtask + 37 项内核测试；覆盖 ACPI 表边界、时钟 / 等待、FIFO、计量、终态与坏输出拒绝。该轮早于最后 PIC 路由修正及验收器 APIC 字段补充，未重跑；后两项由最终 check / VM 验证 |
| 最终静态检查 | `target/xtask-88709-1792926960747939000-0` 的 check 成功，包括格式、宿主 / 裸机检查与严格 clippy；没有放宽检查或删除失败断言 |
| 最终链接 | dev 全新目录 `target/xtask-95939-1792927441790372000-0`；release / boot-test 在最终 VM 目录独立全新构建。三份 ELF 均经独立结构、入口、段权限、静态链接及 map 核验 |
| 整套 VM | `target/xtask-89574-1792927039413464000-0` 的 test --vm 返回 0，约 360.826 秒；10 正常 + 4 T02 + 3 T03 + 7 T04 + 7 T05 + 8 T06，共 39 次；10 次正常使用同一 release 镜像且每次新 VARS |
| T06 真实行为 | 100 次 IRQ / 主循环进展、IF=0 时 HPET 仍推进；真实 page fault 向量 14 / CR2 / 故障 RIP，真实 double fault 向量 8 / 独立 IST，实际 2 MiB 分配拒绝；65 入队保留 64 / 拒绝 1；非法名称后合法事件继续且设备请求为 0 |
| T06 失败边界 | boot-test 停止真实 HPET 后有限读取报告 CLOCK_STALLED；hpet=off 明确 HPET_MISSING；提交真实只读块请求并隐藏完成观察，2 秒时限先触发，三项 share 和活动 DMA 保留，释放 / 复用拒绝。该注入不声称设备发生物理延迟 |
| 独立证据 | Python 独立核对 39 串口及 QEMU 配置；故障 RIP / IST 与实际 ELF 符号匹配。30 张引导 / 源盘的 GPT、FAT、各 37 个 ESP 文件与摘要通过；4 张 T04 DATA 全盘符合独立补丁期望，6 张 T05 DATA 分区外不变、12 文件与 mtools 的名称 / 长度 / 字节一致；T06 timeout DATA 全盘不变 |
| 负对照 | 从实际正常、page fault、double fault、overflow、clock stalled、invalid input、timeout、missing HPET 日志各制一份损坏副本，独立验收器对原件返回 0、坏副本返回 1，原件保持不变 |

最终 ELF SHA-256：dev `a25e15f4dcc67ffe0563f9d26c2686ba373485fc71f5081e2598212957e16e96`（7739632 字节）；release `19c444cadbf3a4b85595e549ab98317a5fa4c711e4955fb062dcaaa8ed923e99`（385056 字节）；boot-test `b85846d4e4477745cd5ed3cae8f0ec1bd2ffe234572aa3dd66f2b56edc3c7ad0`（516000 字节）。不以 dev / release 差异推断位级可复现。

**失败与修复：**定向组首次尝试在 `target/xtask-85460-1792926601464696000-0` 的第一个正常用例失败：HPET 和只读存储通过，但 IRQ tick 不推进，按时限报告 `DEADLINE stage=irq-progress`。核对 PIC 状态及固件继承的本地 APIC 状态后，为已选定的单核 PIC 模式显式清除 IA32_APIC_BASE enable 位并读回验证，拒绝 x2APIC；没有放宽等待或成功条件。修正后十次正常与全部独立用例通过。为保持额度，没有再跑完整九例定向组，而在最终全套中覆盖全部九类 T06；累计为一次失败加 39 次成功，共 40 次启动。沙盒中另一次执行在 Docker 前置阶段被拒绝，未创建盘或启动 guest；同目标取得最小权限后继续。构建期间编译 / clippy 失败及修复输出同样保留。

**资源与留存：**新增 42 张 1 GiB raw（逻辑 45097156608 字节），raw 实际分配 121176064 字节；69 份稀疏 FAT 分区材料实际分配 184008704 字节，合计 305184768 字节（约 291 MiB），低于 1.5 GiB。其他源码 / 构建 / 日志 / 证据归档时 2091692032 字节（约 1995 MiB），低于 2 GiB；统计包含失败构建目录，扣除共享 target/debug 中修改时间早于本次提交且本轮未改写的既有缓存 281092096 字节，清单单独保存，修改过的共享文件按整文件分配计入。监督脚本累计命令执行约 436.525 秒，未触及 60 分钟总执行停止线；独立读盘与归档另记。529 个记录 PID、44 个唯一容器名确认无残留；旧 118 张盘的大小 / 修改时间未变，本次全部盘、VARS、成功与失败材料保留。

小型证据目录为 `evidence/t06-runtime-20261025-5a52758/`：包含初始失败与最终候选两份源码清单 / 副本、运行监督报告、1234 份小型日志 / 元数据复制、三份 ELF / map、独立时钟 / 异常 / 磁盘 / 依赖核验、负对照、清理报告及 SHA256SUMS。大盘与完整构建留对应 target 目录，不复制进证据或提交。证据归档时 1564 条摘要逐项复核，验收后文档及注释差异另存收尾记录，不改写运行候选。

**结论边界：**T06 / AT04 在固定平台本地通过，旧 T02–T05 全套回归及独立存储证据保持通过。未进入 T07，不包含图形、正式 FileService / shell、长任务 UI 响应性、正常电源、安装、真机、远端 Linux Actions 或完整 AT23。本轮实现与结果文档随后提交为 6a6616e，未推送；该提交不改变本节运行候选的内容身份。M0 已有证据保留，任务状态与下一步只在 current 维护。

### 2.18 T07 图形、输入与窗口验收

2026-10-29（Asia/Shanghai），基于 `e1b72f4798744e994b24d6c72fa9f65bebf4a89a` 的未提交工作树实施 T07。冻结运行候选为 `evidence/t07-runtime-20261029-e1b72f4/candidates/final-runtime/` 的 190 文件副本，清单 SHA-256 为 `e0591c4d94fb952b8dcec6c5b8a8945be289cd403123f5238d326c976c476073`。验收后仅同步文档、两份仅宿主许可的复制格式和忽略目录证据，未改运行逻辑；不把候选称为干净提交。

**实现与依赖：**共享 kernel lib 包含 Surface / Format / DrawTarget、类型化输入及校验页；裸机侧保留已验证 framebuffer 描述，复用映射并检查可写地址，专用页面所有者管理规范 RGB 后台及前后哨兵，逐字节 volatile present 不写 pitch padding。1024×768 像素 3145728 字节，实际分配 770 页 / 3153920 字节，堆仍为 1 MiB；真实 800×600 另验。i8042 Set 2 / 三字节鼠标初始化、ACK / RESEND / 超时与 IRQ 队列有界；丢失区间丢弃，释放修饰键 / 按钮状态，最多一次重初始化，失败输入离线。常规 release 不接受 boot-test 注入。

引入四个批准直接版本及用户另行批准的可选直接 sdl2 0.38.0；新增 46 个 registry 包，原有包版本、来源、摘要与依赖关系保持不变。包体 15349696 字节；9 个内核闭包包及字形声明进入 ESP，37 个仅宿主闭包包不进入裸机。具体来源、精确版本、MIT / Zlib / 字形声明及复制格式说明见[许可与来源](../governance/licensing-and-sources.md#t07-图形输入与宿主依赖)。本机 SDL 兼容层 / SDL3 未安装或升级，GUI 只设置子进程链接参数。

| 验证层 | 实际结果与证据 |
| --- | --- |
| 静态 / 宿主 | 最终 check（fmt、宿主 / 裸机 check、严格 clippy）通过，`target/xtask-9198-1793275130479431000-0/`。74 项宿主测试通过：26 xtask、48 kernel，`target/xtask-10038-1793275141015138000-0/`；host-runner 无单独空占位测试，复用共享行为测试并执行真实画面比较 |
| 链接 / 依赖 | dev 全新链接目录 `target/xtask-9197-1793275149438726000-0/`；最终 VM 的 release 与 boot-test 也各自全新构建。Python 独立 ELF 结构、入口符号、LLD map 与 system file 核对通过；完整 `--all-features` metadata 核对 80 包（3 本地、76 registry、固定 fatfs Git），默认 metadata 只含 66 包，不能拿它否定可选 GUI 锁闭包 |
| 最终自动 VM | `target/xtask-11255-1793275199553979000-0/` 一次成功，约 470 秒；10 正常 + 4 T02 + 3 T03 + 7 T04 + 7 T05 + 8 T06 + 8 T07，共 47 次。正常首例完成 Shift / Caps / Ctrl / Alt、Tab / Enter、四边夹取、点击、128 字节拒绝和清空后新输入；其余正常覆盖首帧与静止。T07 独立用例验证 800×600、分配 / 描述拒绝、PS/2 超时、损坏流 / 真队列溢出恢复、裁剪及 100 次重画 |
| 独立磁盘 / 串口 | Python 独立复核 47 串口，异常 RIP / IST 对应实际 ELF；38 张引导 / 源盘的主备 GPT、FAT、各 47 个 ESP 文件和摘要通过。4 张 T04 DATA 全盘符合独立补丁预期；6 张 T05 DATA 分区外不变、12 文件名称 / 长度 / 字节符合独立合成预期与 mtools；1 张 T06 timeout DATA 全盘不变 |
| 独立屏幕 / 负对照 | 全套 27 张实际 PPM，通过独立角点、色块、BDF 字形行、裁剪和指针像素断言，并逐字节匹配同状态宿主画面。50 份坏串口副本（缺释放 / reset、伪造 / 重复 READY）被验收器拒绝；每个有屏幕用例的实际指针像素损坏也被拒绝。静止帧不持续增加，计量稳定，仍不宣称完整 AT18 |
| SDL 窗口 | `target/xtask-95879-1793274217428798000-0/`：实际 a / Shift+A、Tab / Enter、Esc 清空后 b、相对指针与左键释放，最终关闭窗口。私有应用包便于窗口工具识别，未安装到系统。该成功窗口 392.454 秒，连同早期窗口共 472.156 秒，低于 600 秒 |
| Cocoa 窗口 | `target/xtask-20770-1793275731172649000-0/`，216.053 秒。复用固定 QEMU 的逐字节相同私有应用副本与固定固件路径，未修改 QEMU；QMP 仅取屏幕，输入来自真实窗口。普通键 / Tab / Esc 由窗口工具操作；工具的鼠标动作未进入 guest 后由项目所有者实际操作补验，124 帧、17 次键盘释放、左键按下 / 释放各 1、激活 4，指针从 512,384 移到 259,346，未丢事件。两张实际 PPM 独立断言及宿主比较通过，满足条件后自动结束 VM |

ELF SHA-256：dev `5f814811b8ebe7474e04706042f2a4c69453114c1e04da5af0962d7db831d9b1`；release `c285fe6cf30c71327d7b40e17591ef69ff38049c12220482d03e84ef345a586b`；boot-test `04a28b839aefb9b318cda7cde79c3af669ef9f944a136b99b4b482d0804a8f3f`。对应报告为 `dev-elf.json`、`release-elf.json`、`boot-test-elf.json`。

**失败与修正：**初始九例定向组在第 5 次启动的 PS/2 超时注入处失败：隐藏 ACK 后仍把同事务 BAT 当普通回复，得到意外回复而非超时。修正测试观察为隔离已隐藏 ACK 的整段回复，保持真实命令发送和原始时限。随后定向组的损坏输入恢复成功，但溢出用例只收到 193 字节，未填满 256 项队列；改为两个完整按键一批，等待真实 raw 接收计数前进。下一次发现读取半条串口记录，修正为只解析换行完整的记录。最终单例溢出通过，所有九类在最终 47 次套件中通过；不能声称独立九例分组另行完整通过。定向累计 9 次（6 成功、3 失败），失败日志和源码快照均保留。未减小软件队列、放宽成功条件或增加重试预算。

SDL 沙盒尝试报告无显示器，申请同一已授权目标的最小沙盒外权限后可用；早期未带应用身份的窗口退出后，用私有应用包完成交互。Cocoa 的工具合成 Shift 组合未产生 guest Shift，工具合成鼠标也未产生移动 / 按钮事件，不能把它们记为成功；实际鼠标由所有者补齐。窗口检查不声称逐项人工重放全部修饰键（完整 QMP 链路已覆盖），SDL FocusLost 分支有实现和共享 reset 测试，本次窗口会话未单独演练失焦。

**资源与收尾：**累计 57 次 VM（9 定向尝试 + 47 最终 + 1 Cocoa）与 59 张新增 1 GiB 稀疏 raw，额度正好用满。raw 实际分配 158531584 字节；含 103 份 FAT 分区材料共 432422912 字节，约 412 MiB，低于 2 GiB。其他修改 / 新增缓存、构建、源码及证据收尾约 3.3 GiB，低于 4 GiB；精确最终值见 `resource-audit.json`。xtask 已记录自动子进程 680.595 秒、GUI 688.209 秒；独立脚本与早期直接依赖探针另计，整体验证未接近 60 分钟自动 / 80 分钟总上限。782 个记录 PID、任务容器及私有 socket 已只读核对无残留；不凭历史 PID 杀进程，不清理其他任务资源。盘、VARS、缓存、成功和失败证据全部保留。

**结论边界：**T07 及 AT07 的基础显示 / 输入子项在固定本地平台通过，既有 T02–T06 回归保持通过；AT07 整体、M1 工作流、正式文件服务、终端、双面板、正常电源、安装、真机、远端 Linux Actions 与完整 AT23 均不在本批完成声明内。运行产物和独立脚本位于 `evidence/t07-runtime-20261029-e1b72f4/`，实际目录见本节和机读报告；PNG 预览由真实 Cocoa 交互后的 PPM 无损转码，非设计稿。当前 Git 与下一步统一由 current 维护。

### 2.19 旧测试盘清理

2026-10-29 按所有者要求，在 T07 本地验收后清理 10 组已被后续完整回归覆盖的 T02–T06 成功运行材料。仅删除 `target/xtask-*` 内清单列出的 103 张 `disk.raw` 和 170 份 `data.fat` / `esp.fat`，共 273 个普通文件。逻辑容量 207402584064 字节，删除前实际分配合计 **699531264 字节（约 667 MiB）**；这是文件分配量，不把逻辑容量或 APFS 卷可用空间变化当成实际回收值。

删除前核对每组 `steps.txt` 的成功终态及 evidence 中的归档副本，逐路径确认解析结果、普通文件类型、inode、大小和修改时间，并检查无已打开文件描述符。删除后确认清单文件全部不存在，另外 231 份 target 盘 / 分区材料的大小、修改时间和 inode 未变。精确路径、原分配量、运行记录摘要及结果见忽略目录 `evidence/disk-cleanup-20261029/manifest.json` 与 `result.json`。

T07 全部盘、独立 M0 重放、失败运行、T01 工具验证盘、固件 VARS、配置、源码、日志和独立报告继续保留；未清理依赖、构建缓存或其他项目。前面各节“全部保留”为验收当时快照，本节更新旧成功盘的当前留存状态。已删盘不能再直接读回，历史报告仍是当时验证记录；如需复验须按历史源码及输入重新生成并取得新的 VM 资源授权，不能恢复原始运行现场。

## 3. 安装影响与清理边界

| 动作 | 写入与资源影响 | 恢复 / 清理方式 |
| --- | --- | --- |
| 安装指定 nightly 和组件 | 写入 `~/.rustup/toolchains/` 与下载缓存；可能占用数 GiB，实际以下载清单为准 | 保留现有 1.96.0 / 1.97.1 与默认设置；仅在确认未被其他项目使用时移除本任务新增工具链 |
| 正常 Homebrew 安装 QEMU | 写共享 `/opt/homebrew`；23 项新增、4 项升级、3 项复用；默认 OpenSSL 与共享 CA 发生变化，详见第 2.3 节 | 保留受影响旧 keg / 配置及状态记录，不自动清理；卸载 QEMU 不等于回滚依赖，按第 2.4 节逐项评估恢复 |
| 构建 Linux 工具镜像 | OrbStack 镜像存储、构建缓存、包索引及专用输出目录产生写入 | 记录项目镜像 ID / digest 和容器 ID；退出本任务容器，仅清理归属明确的资源，不执行全局 prune |
| 后续制盘 / VM 验证 | 每份 raw 逻辑容量 1 GiB，副本数与实际占用另记；每次运行有独立 VARS 和日志；guest 基线 1 vCPU / 512 MiB，宿主另有仿真开销 | 仅使用仓库忽略目录内的任务普通文件，不挂宿主物理盘；结束关闭任务进程，保留失败副本时记录路径 |

本次预演对应的 27 项新增 / 升级制品，registry 大小元数据合计下载 **155,783,479 字节**、安装大小 **879,078,127 字节**，约 156 MB / 879 MB；不包含三项既有依赖，不需要第二套 Homebrew / portable Ruby。QEMU 本体为 113,262,802 / 728,988,917 字节（下载 / 安装）。已核对各平台 manifest 的制品 digest，未下载软件归档；这是制品容量估计，不是净磁盘增量，另需保留旧 keg、下载缓存、临时目录和恢复材料空间。

Rust 与 QEMU 下载安装的 10–30 分钟仍只是未测量预算。此前 Debian 构建由 BuildKit 报告基础元数据阶段 37.2 秒、安装 RUN 阶段 29.2 秒；这些分阶段时间受网络与缓存影响，不是完整构建计时或固定预算。安装前列出包管理器实际解析的新增 / 升级项、总下载量与磁盘需求；不运行全局 upgrade，不改变 shell 配置或现有 Docker 默认 context。

Limine 二进制与 Rust crate 已按第 2.11 节单独授权配对；T03 内存 / 设备、T04 GPT 与 T05 FAT 依赖已按第 2.12–2.14 节单独授权引入，T06 的既有 x86_64 中断 ABI 特性按第 2.17 节授权启用；T07 图形 / 输入及宿主 runner 的依赖与资源另见 [T07 实施契约](../mvp-plan.md#t07-实施契约与验证方案)，授权和执行状态统一见 current，不包含在以上宿主工具安装集内。T01 最小 no_std 编译不需要先安装所有 T02–T05 依赖；形成引导镜像前必须完成 Limine 协议修订、资产哈希与许可核对。当前 raw / UEFI 方向不预装 xorriso、NASM、完整 LLVM 或 Linux Live 介质，确有实现需要时再补具体版本。

此前仅在本地工具镜像引入 Debian 基础系统与五包，仓库保存构建配方及文本清单，不提交第三方二进制。已按[许可与来源策略](../governance/licensing-and-sources.md)登记；Rust nightly 已安装并记录来源 / 随包许可，见第 2.9 节；QEMU / EDK II 已在本机安装并读取许可，见第 2.8 节。QEMU formula 的 GPL-2.0-only 不覆盖所有附带固件，EDK II 依据其 `edk2-licenses.txt` 单独记录。本表不是分发许可结论，也不承诺打包这些宿主工具。

## 4. 分层验证方案

以下是分层验收要求。Debian 签名 / 快照策略与模拟求解已有第 2.5 节证据，工具镜像安装及 CLI 可运行性已有第 2.6 节证据；工具文件操作、独立读取及五类损坏检测已有第 2.7 节证据；QEMU 安装后的 CLI、链接、OpenSSL 与固件核验已有第 2.8 节证据；Rust nightly 的版本、组件、target 与链接器核验已有第 2.9 节证据。第 2.10 节补齐 T01 工程编译、宿主检查和独立 ELF 证据，第 2.11 节补齐 T02 真实引导，第 2.12 节补齐 T03 内存 / PCI / HAL，第 2.13 节补齐 T04 块存储 / GPT，第 2.14 节补齐 T05 FAT，第 2.15–2.16 节记录 M0 收口，第 2.17 节补齐 T06 异常 / 时钟 / 循环，第 2.18 节补齐 T07 图形 / 输入 / 窗口；其余未执行的项目不能勾为通过。安装完成与业务实现完成分开验收。

| 层次 | 方法与通过条件 | 失败处理 / 证据边界 |
| --- | --- | --- |
| 安装前身份核对 | Rust 日期 manifest 与组件可用；正常 Homebrew 重新预演匹配第 2.2 节版本 / revision / rebuild / bottle 哈希和 23 / 4 / 3 动作，明确共享默认 OpenSSL / CA 影响；Debian 平台 digest、快照签名经实际 apt / sqv 策略验证，包身份及求解匹配 | 缺包、哈希不符、版本漂移或额外安装 / 重装项则停在该项；不自动换源或选 latest，先列出差异 |
| Rust 工具可运行 | 显式 `cargo +nightly-2026-10-13 -V` / `rustc +nightly-2026-10-13 -Vv`，核对已装 target / 组件及 rust-lld | 避免 `RUSTUP_TOOLCHAIN=1.96.0` 干扰；不改用户全局默认；版本输出仅证明工具可执行 |
| QEMU 功能枚举 | `qemu-system-x86_64 --version`、`-machine help`、`-accel help`、`-display help` 与 `qemu-img --version`；记录实际支持的精确 pc-q35-* 名称、TCG 和显示后端 | 已实测支持 pc-q35-11.1 / TCG，显示交互未验证；若缺所需后端，不降低为另一架构 / 加速器冒充通过 |
| 宿主链接与共享影响 | 安装授权后核对 QEMU / libssh 的 `otool -L`、递归库 / LC_RPATH / 脚本来源，核对 libssh 使用 OpenSSL 4、既有 OpenSSL 3 路径仍有效；共享收据 / 链接变化与计划一致，按风险补共享工具及 CA 影响检查 | 元数据 ABI 标记不能替代运行证据；不能全局屏蔽反向依赖检查来宣称安全；失败时保留共享差异并按第 2.4 节处理；先确认归属，不盲目回写或批量卸载 |
| 固件核对 | 读安装包 descriptor、许可证、两个文件尺寸与 SHA-256；代码只读，VARS 每次从模板复制 | 描述文件与实体路径不一致、架构不符则失败；不能用“有 .fd”证明可启动 |
| Linux 工具可运行 | 在项目专用 ARM64 容器读取 dpkg 精确包版本、sgdisk / mkfs.fat / mtools 版本；记录基础与最终镜像 digest、完整包清单 | 不进入其他项目容器；没有目标包不得报告环境就绪；不使用 privileged 或透传宿主设备 |
| 工具制镜像验证 | 只对新建普通文件生成规划中的 1 GiB / 512 字节扇区 GPT、128 MiB FAT32 ESP 和 DATA；检查主备 GPT、边界及 FAT；合成文件复制后用独立读取路径对照内容哈希 | 不把制盘工具成功当作内核持久化通过；读盘前确保无活动 guest，不自动修复损坏样本 |
| T01 工程验证 | 实现获授权后建立真实 Cargo workspace 和入口；干净输出目录编译 x86_64-unknown-none 最小 no_std 内核，确认 ELF x86_64；宿主与内核依赖图隔离，记录实际 features / lockfile | 不能用宿主 cargo check 或版本输出替代裸机编译；无入口时不建占位成功检查 |
| T02–T05 / M0 | 固定精确机器名、CPU / 设备参数、TCG、固件及镜像哈希；取得引导、写入、flush、重启读回、宿主独立校验与失败证据 | 依 MVP 的 T / AT 执行；UEFI 启动画面、最小编译或模拟器通过均不能替代 M0 完成 |

ARM64 宿主模拟 x86_64 使用 TCG，硬件虚拟化要求宿主与 guest 架构匹配；不以 ARM64 容器可运行推断 x86_64 guest 已验证。[UTM 架构说明](https://docs.getutm.app/settings-qemu/system/)、[QEMU 加速器与能力枚举](https://www.qemu.org/docs/master/system/introduction.html)

## 5. 盘点限制与复核入口

本机事实来自 `uname` / `sw_vers`、`command -v`、rustup installed 查询、已有 rustc / cargo / rust-lld 的版本输出、应用 Info.plist、`file` / `strings`、固件 descriptor / `shasum` / 文件尺寸，以及 Docker 格式化只读 API。新增核对包括 Docker / GHCR registry JSON、固定官方配方与 AST、签名快照索引、包路径 HEAD、本地安装收据，以及固定 Debian 容器的 sqv / APT 更新、负向检查、模拟求解和派生工具镜像的包体 / 安装 / CLI 证据。2026-10-19 补充 GPT/FAT 普通文件制盘、宿主独立解析、原始 CRC / 文件哈希及五类损坏副本的实际证据。UTM 动态库字符串不是独立 QEMU 的运行结果；本机性能与完整产品行为仍未验证；nightly 的 T01 实际编译 / 链接见第 2.10 节，T02 guest 引导见第 2.11 节；用户安装后的 QEMU / 固件核验见第 2.8 节，Rust 工具链核验见第 2.9 节。

早期 Homebrew 的 `list --versions` 即使设置 NO_AUTO_UPDATE 仍尝试刷新 API，代理失败后改用只读 Cellar；这不是安装清单查询成功的证据。2026-10-19 采用任务专用缓存、官方 API 和实际 dry-run 成功取得当前计划。首次联网受沙盒代理限制，为同一元数据目标取得权限后成功；未安装工具或修改全局代理。Docker API 和 sysctl 的早期权限限制仍按原记录解释，不据此补填硬件性能。

`install --dry-run` 仍经过 `perform_preinstall_checks`，可能创建缺失目录或准备 portable Ruby，不能一概视作零写入命令。安装前预演先读本机源码，确认所需共享目录与 portable Ruby 已存在后执行；任务缓存、日志和临时文件产生写入。预演前后 Homebrew commit、全部 40 份安装收据 SHA-256、424 条已采集共享链接与目录存在性完全一致，当时 qemu Cellar 和 `/opt/rrt01` 均未出现。此检查覆盖上述对象，不能扩大为全磁盘字节不变证明。

本地公开证据保存在忽略目录 `evidence/t01-homebrew-20261019-19ad0a5/`：官方 API、七份配方原文、27 份 registry manifest、dry-run / info 输出、已装库链接、前后基线与摘要，连同缓存实际占盘约 40 MiB。临时脚本仅用于安装前盘点，不是项目安装入口；该预演没有下载 bottle / 源码归档、读取钥匙串、安装 / 更新 / 卸载 / 链接操作或后台资源。旧 AST / 专用前缀分析保留为历史；当前方案见第 2.2–2.4 节，安装后的事实以第 2.8 节为准。
