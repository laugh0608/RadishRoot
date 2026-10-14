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
| Linux 基础环境 | `debian:trixie-20261005-slim`，`linux/arm64/v8` | registry index → ARM64 manifest 映射与响应哈希已核对，见第 2.1 节；未拉取镜像层 |
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

这些归档未下载，哈希来自官方文本元数据。Debian 日期标签映射已补齐，未来拉取应使用下述 ARM64 digest；不能把元数据核对视为基础镜像已经下载、启动或可信执行。

QEMU formula 原文现已取得并计算内容哈希，与 API 的 `b8fa843b66a8879edf2f4c2440a4990557762764fc7f46767a26929d355215c7` 一致；固定来源见第 2.3 节。普通 `brew install qemu` 会随 formula 变化，不能视为永久精确安装命令。

### 2.1 Debian registry 映射与工具包影响

通过 Docker Hub 的匿名 registry 读取日期标签 index，再按平台 descriptor 读取 ARM64 manifest；两份响应均重新计算 SHA-256，与 registry 标识 / descriptor 一致。读取的是 JSON 元数据，没有请求镜像层内容。

| 证据 | 精确值 |
| --- | --- |
| 标签 | `docker.io/library/debian:trixie-20261005-slim` |
| 多平台 index | `sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f` |
| 选定平台 | `linux/arm64/v8` |
| 选定 manifest / 后续拉取身份 | `docker.io/library/debian@sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850` |
| 上游构建来源 revision | `cf1f4a45447842b45e9952e0f018ae734a7341c7` |
| layer 元数据 | 30,200,282 字节；`sha256:bbeda6b4abb749f743f4547ef5a28070a14de8caef211daa5afb78d5b5fa21ba`；未下载 |

来源为 [registry 日期标签](https://registry-1.docker.io/v2/library/debian/manifests/trixie-20261005-slim)与 [ARM64 构建包清单](https://github.com/debuerreotype/docker-debian-artifacts/blob/cf1f4a45447842b45e9952e0f018ae734a7341c7/trixie/slim/rootfs.manifest)。registry 查询需要匿名 pull scope token，token 不写入文档或日志。

对照基础包清单与已验证的 `20261005T000000Z` trixie main ARM64 Packages 索引，按 `--no-install-recommends` 推导以下五包新增；基础 libc6、libgcc-s1、libstdc++6、libtinfo6、libuuid1 等已满足这些包的必需版本。未运行 apt 求解器或容器，因此这是元数据推导，不是安装事务保证。

| 新增包 | 精确版本 | 包下载字节 | Installed-Size（KiB） | 包 SHA-256 |
| --- | --- | --- | --- | --- |
| gdisk | 1.0.10-2+b1 | 216476 | 1089 | `b6b48390e63bff9d18b32be3270910fab83f8c14117d49ada13cba8a2a75c7e9` |
| dosfstools | 4.2-1.2 | 130564 | 374 | `7ec5c86659561b5e1b412227e0e3fab9dc17e4214274cb1b4cb3b5c5f8c288e9` |
| mtools | 4.0.48-1 | 192396 | 504 | `296cf0a6b06433520effc30542a695307f4abf0d377f94d8a22dc8bd1dd4785f` |
| libncursesw6 | 6.5+20250216-2 | 124048 | 568 | `72121aeb08be2d915da321ddfae74f62a0764286f336279b2c2609ba07496d6d` |
| libpopt0 | 1.19+dfsg-2 | 42808 | 257 | `b58ba178effc24280c4c6be3cf2c93a34607e18ad76b2f74b2de818136699076` |

五包合计 706,292 字节下载、2,792 KiB 安装大小；不含基础镜像、包索引、缓存和容器存储开销。groff-base、libgpm2 属 recommends，floppyd 属 suggests，不纳入该安装集。五包在快照 main 索引中的版本、架构、大小和 SHA-256 均与上表一致；其快照 `.deb` 路径逐项 HEAD 返回 200，Content-Length 与表内一致。没有下载 `.deb`，因此仅证明签名索引声明与路径可用，包体哈希须下载后再核对。

#### 快照签名与索引证据

基础镜像默认使用移动的 trixie / trixie-updates / trixie-security 源。选择上游 [snapshot 源模板](https://github.com/debuerreotype/docker-debian-artifacts/blob/cf1f4a45447842b45e9952e0f018ae734a7341c7/trixie/slim/rootfs.debian-sources-snapshot)的 `20261005T000000Z`；main / updates 使用 `archive/debian`，security 使用 `archive/debian-security`。固定基础镜像 digest 不能代替此软件源冻结。

本机未发现 gpg / gpgv；没有为核验安装它们。临时核验程序按 OpenPGP v4 格式解析实际的 RSA 签名，用已有 Python cryptography 验证 RSA PKCS#1 v1.5；校验主钥指纹、签名子钥绑定、子钥交叉认证与子钥到期时间，再验证三个 InRelease 的规范化正文。另用已有 OpenSSL 3.6.4 的 `dgst -sha256 -verify` 对提取的三个签名独立调用复核，均返回 `Verified OK`。正文改动和签名位翻转的负向样本均被拒绝。未执行下载的 formula 或安装脚本。

信任锚来自 Debian 官方 HTTPS 公钥文件，主钥 / 子钥指纹同时对照 [FTP master 公钥页](https://ftp-master.debian.org/keys.html)和 [2025 年公钥公告](https://lists.debian.org/debian-devel-announce/2025/04/msg00001.html)。这是对这些固定签名的密码学复核，**不是 apt / sqv 的完整策略验证**；没有建立离线预置信任锚、运行通用 OpenPGP 验证器或核验所有并列签名。基础清单提供 debian-archive-keyring 2025.1、apt 3.0.3、sqv 1.3.0-3+b2，安装授权后仍须通过这些实际组件核验源与求解；临时解析程序不进入安装信任路径。

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

五个新增包没有声明 Pre-Depends、Conflicts 或 Breaks；必需 Depends 逐项检查后，已有 libc6 2.41-12+deb13u4、libgcc-s1 / libstdc++6 / gcc-14-base 14.2.0-19、libtinfo6 6.5+20250216-2、libuuid1 2.41.5-0+deb13u1 满足版本限制，包括 libncursesw6 对 libtinfo6 的精确等号要求。新增包所需的这些基础依赖递归仍由既有基础集合满足。updates 没有这五包，security 也未提供它们的其他候选；因此元数据结论仍为新增五包、无需因这五包升级基础库，不等于已经运行 apt 模拟事务。

固定基础包清单 SHA-256 为 `d36f1afce31468d16ff66da178751c73bfd5347271f8411015f8b937cd3b1132`。其中没有 ca-certificates，不能把宿主 HTTPS 查询成功推成容器 HTTPS 已可用；实际构建须遵循已审阅源模板的传输方式与签名校验，若改用需要新增 CA 包的方式，应重新记录依赖影响。新增包须纳入安装清单与授权范围。

### 2.2 QEMU 传递依赖与 OpenSSL 冻结方案

选定方案：以 QEMU 11.1.2 的 `arm64_golden_gate` bottle 构建记录为候选基线，将其 29 项运行依赖与 QEMU 本体一起按精确版本、formula revision、bottle rebuild、平台和制品 SHA-256 固定。OpenSSL 链固定为 **libssh 0.12.2（revision 0）+ openssl@3 3.6.4_1**；不引入 openssl@4，不用当前 libssh 0.12.2_1 替换。它们是宿主仿真器依赖，不是 guest 内核依赖。

原先按当日移动 formula API 推导的“23 项新增、3 项升级、4 项同版本”用于说明直接安装最新 formula 的影响。该图含 glib 2.90.1、libpng 1.6.59、libssh 0.12.2_1 / openssl@4 4.0.3，与选定 bottle 的构建记录不同，不能作为本方案的冻结表。

证据来自 [QEMU 11.1.2 registry index](https://ghcr.io/v2/homebrew/core/qemu/manifests/11.1.2)的 `sh.brew.tab` 与 SBOM supplement，再逐项读取依赖 index 并对照 bottle digest。QEMU index 响应 SHA-256 为 `99ead5c77be214e46df7df8be08c89bbb786442adf9ea635e75c27a2d028a0c2`，其 ARM64 manifest descriptor 为 `sha256:a6ff912513ddad07b1a72aae63170207844ecfda906ba32c4ec6967b1893df4e`。已核对 29 项依赖的制品 SHA 与根记录相符，递归包名未超出下表；这不等于实际动态链接已经通过。

除 ca-certificates 为 `all` 外，表内平台均为 `arm64_golden_gate`。对应 registry 仓库为 `ghcr.io/v2/homebrew/core/<包名>`，`openssl@3` 映射为 `openssl/3`；index tag 为精确版本，rebuild 非零时追加 `-<rebuild>`。bottle 内容使用 `/blobs/sha256:<表内哈希>` 标识，版本标签只用于查找，不能覆盖内容校验。

| 包 | 精确版本（含 formula revision） | bottle rebuild | bottle SHA-256 |
| --- | --- | --- | --- |
| qemu | 11.1.2 | 0 | `a399faaf2f8fb45d3b1f35704041a9104f7f717e9fda1b12d4deac9e919d0d33` |
| ca-certificates | 2026-09-25 | 0 | `81804ad8a70a7a8d2a0a964d106765ff88716b310decaeffb4b4895a3252186a` |
| capstone | 5.0.9 | 0 | `6b31fe379681e2c0778e30b3516cdfc2cfdff6424f666463643f1507039daf0b` |
| dtc | 1.8.1 | 0 | `92ee4960b1643f3ba722d3d0cd9eb31af546215c7eaf858e76e2c69dd2c7afe8` |
| gettext | 1.0 | 2 | `4db034aa2ae4b1f18d86906707ece14c895198090ad028d2fb1ae4fbeb3ca6fd` |
| glib | 2.90.0 | 0 | `07422c68ee1c3d01ba6ac6296668e82cad2ee21c9466e60e79a70da2c5947207` |
| gmp | 6.3.0 | 1 | `5116a8201d8e044aef7d486da7e57f1e4dd2d8554ea6b540b44e45c0804f8f6f` |
| gnutls | 3.8.13_2 | 1 | `366c9b9c0a6b246af58952c4bb5ac921ec5be87bdc23c0b9d8b2ca97b1a3d996` |
| jpeg-turbo | 3.2.0 | 1 | `7bc0f7c007a73c68da8c11055e836de66c2814c1b0ca0302da9a6317199fd37a` |
| json-c | 0.19 | 1 | `6b45746f31780a84ae3ed11d056bd101d1b0a6d0f16bf91aad2d35a18f6107dc` |
| libidn2 | 2.3.8 | 0 | `bacc89595e4e8d2a4e14eb71730ae96a1ca68830068a741c59fc363a355ef1bc` |
| libpng | 1.6.58 | 1 | `02e4e023512c6bf086f139717bcb90fb53ece5fbab3ab4354b0d5fed25afa5db` |
| libslirp | 4.9.5 | 0 | `9c0233380116a46e7d292a7f0b1b3084214f399ae839806bcff0018f581da405` |
| libssh | 0.12.2 | 0 | `cc4bab002dd48933367fe51da95ce7a4140e8e06aa01dad06c48d057414de073` |
| libtasn1 | 4.21.0 | 0 | `68d1748171848565af7a083ca92b834ab1fe4ac7832140b1cd22fd1fcb072fd7` |
| libunistring | 1.4.2 | 1 | `57307eb89a02e09aa4de70e09391994dfc9520d9f101ec7e64f9af21dcea2420` |
| libusb | 1.0.30 | 0 | `b462926c2314b5bb7d1a6721f9a3cf7336a4166ba67024b4bbd0226de457132a` |
| libyaml | 0.2.5 | 0 | `afaaa073d70a7e69f0ff53dcdf1ee08d1ac594eb2e458afa665403adf4209078` |
| lz4 | 1.10.0 | 3 | `53ea7532077fe43abfe3d6e0d9991ebc8370e9edcc508f997c92902d60c8b151` |
| lzo | 2.10 | 0 | `928ed0070e25fe8082e24daa3f967657be0c1c5c72c14a3ac4cd498874eeec11` |
| ncurses | 6.6 | 0 | `36a1f053f1be8ce8dce8a92b68395fb1483b077d313077e9c8f4b20b6f712602` |
| nettle | 4.0 | 0 | `42ff85a054b4a265542175dd31f4a515e0a3e7d8113cfddacc5472555a036bc8` |
| openssl@3 | 3.6.4_1 | 0 | `037d39aeb53c636e392fa670fa74c2a94b51b69521a4fb4fce4d0ea7500f5f94` |
| p11-kit | 0.26.5 | 1 | `a7c4679040141c7c8cb0861e164126871f7e0eb485df6da5ebecc3665e185ce2` |
| pcre2 | 10.49 | 0 | `5cac44323243138490ca74a2499fd347fa836df6b5fffa789944be12110e91ff` |
| pixman | 0.46.4 | 1 | `918ff532bf70bc121376f4b51420f44fc1abd0b9c519d7f8040f6ad9d304bc02` |
| snappy | 1.3.1 | 0 | `7f20ac629eaf9a34a0e4d0e1fe60a933d42ececf3d956988fa8b24916e467c85` |
| vde | 2.3.3 | 0 | `972ce0f14fb0fe92932f45b887019b228ac4529f06d0a95eeb88ece1359d76ba` |
| xz | 5.8.4 | 0 | `b2b5d6523153be05c20bcc51cd358ea3dd1f4bf893748fdedb779ab508ea63ce` |
| zstd | 1.5.7_1 | 0 | `7fb4af03304cc4897326168cdcd53290311c5543150f5502a61c3e84b6d62765` |

这里将根 bottle 的依赖记录作为**精确候选选择**，不是声称每个叶子 bottle 都在完全相同的版本组合下构建。例如部分叶子记录较旧的 pcre2、ca-certificates 或 gettext rebuild；采用上表根集合统一版本后仍须做动态链接和功能验证，不能只比较版本大小就证明 ABI 兼容。

冻结实施候选仍为项目维护的固定 formula 集合；本轮已补齐原文、commit、内容哈希与静态运行依赖图，见第 2.3 节。原文没有本地改动，也没有创建 tap 或安装器。所有依赖必须解析到同一固定集合，bottle 必须匹配上表；禁用移动 API 不能替代固定引用。本机安装器源码暴露的 tap / keg 冲突尚未解决，不能把该集合视为已可直接安装。只为本任务命令设置元数据 / 清理控制，不改全局 shell 或用户 core 历史。

安装前须验证该固定集合实际解析的全部运行依赖、资源、post_install 副作用和已有反向依赖影响；若本地配方命名 / 路径使 bottle 不能按原制品使用，或需要源码构建、额外包、版本变更，则停止并更新方案，不通过 `--ignore-dependencies`、手工解压替换共享库或隐藏链接错误完成安装。`brew pin qemu`、关闭 auto-update、`bundle --no-upgrade` 都不能代替上述闭包和制品锁定。[Homebrew 版本维护说明](https://docs.brew.sh/Versions)

安装后的运行身份还须记录实际可执行文件 / 动态库来源、固件哈希、版本化机器名与功能验证结果；后续发现共享 Homebrew 依赖漂移时先报告环境不匹配，不能继续沿用旧验收结论。升级候选须整体更新受影响版本 / 哈希并重跑相关验证，不永久把历史候选当作维护策略。

### 2.3 固定 formula 来源与静态解析核对

下表均为已读取、重新计算 SHA-256 的 Homebrew 官方原文，包含其版本 / revision、bottle rebuild 与第 2.2 节的目标 bottle SHA。26 项来自提交 A；四项使用各自历史提交。它们是与选定制品身份匹配的配方快照，不保证每份文本恰为 bottle 构建时嵌入的 `.brew` 文件；归档尚未下载，不能声称已比较嵌入内容。当前 API 已将 openssl@3 改为 3.5.9（LTS），进一步说明不能依赖移动配方选择 3.6.4_1。

| 来源代号 | 官方 homebrew-core 完整 commit | 使用范围 |
| --- | --- | --- |
| A | `40d747b51828c1d4a020c78bd8eba726e2398822` | 除以下四项外的 26 项 |
| B | `fc37e24fc3f15b020b7c5de326f10fd6c2ee0200` | glib 2.90.0 |
| C | `814131203f42237a7d78d6aa73fde215710eb755` | libpng 1.6.58，rebuild 1 |
| D | `791e964854c43bc881392cc51c07c819f836ee15` | libssh 0.12.2，revision 0 |
| E | `29441026de76b0e256e6444a893967a45f25a6b6` | openssl@3 3.6.4_1 |

| 包 / 固定原文路径 | 来源 | 原文 SHA-256 |
| --- | --- | --- |
| [qemu](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/q/qemu.rb) | A | `b8fa843b66a8879edf2f4c2440a4990557762764fc7f46767a26929d355215c7` |
| [ca-certificates](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/c/ca-certificates.rb) | A | `e7807634f02d41cfad036cdebb546daacd45e118fcb463acc6c6e4f6e07dcf3f` |
| [capstone](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/c/capstone.rb) | A | `74d7afa733f1f748ad4b71e691b519b865f35e07da778f9b1bc111a5f926dfce` |
| [dtc](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/d/dtc.rb) | A | `0996948d0919552f834245870a8fba81871c583becd2c3c16a15544927fc9060` |
| [gettext](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/g/gettext.rb) | A | `9c90f7cf5a45c828e2a66d9339b25b781302cf49316e939d029e6079e1b9d916` |
| [glib](https://github.com/Homebrew/homebrew-core/blob/fc37e24fc3f15b020b7c5de326f10fd6c2ee0200/Formula/g/glib.rb) | B | `7e41107faf209528d2e993afec53ad1006bbe8945772bde43c2295e8a2ef7659` |
| [gmp](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/g/gmp.rb) | A | `a7a712aab26d2b88865ebd9c06f4eeb131c53815f3b6e467578b32ab66ac3b18` |
| [gnutls](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/g/gnutls.rb) | A | `720a0b82acbc9f97fe2950c6b919ea5e9383b491c8ef7b7f06a7f3e16c9b8c07` |
| [jpeg-turbo](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/j/jpeg-turbo.rb) | A | `7e2e90fde4b71193c517e87763b199f3ebf185afcce4b99d82eee7467e761b8d` |
| [json-c](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/j/json-c.rb) | A | `7b6c55b79158a98a41e376a39e05bbee827f425b67654af75db461f79402af37` |
| [libidn2](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/lib/libidn2.rb) | A | `032d48cd71a4c81e9c6a7847b1dc2d3b0a1d1f21a8fa6fead4a7ff1b60df2160` |
| [libpng](https://github.com/Homebrew/homebrew-core/blob/814131203f42237a7d78d6aa73fde215710eb755/Formula/lib/libpng.rb) | C | `36aa425fe741b14fb226159ebfadfc3b75ed0603d7b27675b6cd1ffdeba80d07` |
| [libslirp](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/lib/libslirp.rb) | A | `bb43caff8fd85c3ff9dcf0dd0cda7b56498e45ee1becbe6431230391b9f68ebb` |
| [libssh](https://github.com/Homebrew/homebrew-core/blob/791e964854c43bc881392cc51c07c819f836ee15/Formula/lib/libssh.rb) | D | `35acc436de5d09ce9ead74ce6d3bd0e5df2abeb818015082588567a2637cc863` |
| [libtasn1](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/lib/libtasn1.rb) | A | `8c0814380e9449c0b9b5cfcbf12e7e44e25e581c8f3909f2467c63bb482bcd2d` |
| [libunistring](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/lib/libunistring.rb) | A | `8acac13619748b5c040c866f78955ea15fe51f555d60d494f52beae6ee24ce34` |
| [libusb](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/lib/libusb.rb) | A | `513c20f992711ced872639a391fbb0b3194e397ffddc4968b9f9b1e4f03b48ee` |
| [libyaml](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/lib/libyaml.rb) | A | `dfad02f7f8f65dff0f00fee6ca2d99b59309ad8c7bc6f2148ae60788f2bf8a79` |
| [lz4](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/l/lz4.rb) | A | `b45023020f34ac622d84d10e44f1d972f742d5c741098f6ee828b6ab79bd19e4` |
| [lzo](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/l/lzo.rb) | A | `8acc83503bd65a1e198d16638fe911201c7922cf70b24b5db3015fcd948c54cd` |
| [ncurses](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/n/ncurses.rb) | A | `2e35152ac8c0b209f90373090ad3d3a64d7818fc965d6f58142ab1d9fa95f824` |
| [nettle](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/n/nettle.rb) | A | `8135edd1ccf828e5a9550a965bd5f11a862ad028335f451838998f3e5215e4f2` |
| [openssl@3](https://github.com/Homebrew/homebrew-core/blob/29441026de76b0e256e6444a893967a45f25a6b6/Formula/o/openssl@3.rb) | E | `e5c03ebbc8f6dc2a48690c9a293b2dcf71857a63a9a33d0a5c405539634bc30d` |
| [p11-kit](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/p/p11-kit.rb) | A | `f417ea91e494ec02cc2c79284c80dd4f1b214ba6678a0669af531243aacdaded` |
| [pcre2](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/p/pcre2.rb) | A | `2dcb74b4bc19dc81ace51feac7bb626b679c7b730eca1d3492d73fe822134fb8` |
| [pixman](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/p/pixman.rb) | A | `a9957ff4b6518062d2e2697b5ff8b470b06e5a73f8f9076c9d3e7d1c8dbc1d85` |
| [snappy](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/s/snappy.rb) | A | `65ad62d494f32ebad469606114480e5aeb6e88cac51d6bd5067e7cfed4c86959` |
| [vde](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/v/vde.rb) | A | `1d48d707aaff08a2874c7180ff04df89f971210e6b60a7b5894257e6c5948e7f` |
| [xz](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/x/xz.rb) | A | `e5b8bbf09dccb4a1f2ee333ed65b13ee773350a26735ef594ca859d2cf85248e` |
| [zstd](https://github.com/Homebrew/homebrew-core/blob/40d747b51828c1d4a020c78bd8eba726e2398822/Formula/z/zstd.rb) | A | `e40f1de5b4e53faf64cad3d24dd74e28948d674caf96b3229310ce78e66b030e` |

使用本机已有 Ruby 的 Ripper 解析 30 份文本的 AST，未执行配方 Ruby 代码；按 stable、macOS ARM64、使用 bottle 的条件展开，排除 HEAD / Linux / Intel 分支以及 build / test 依赖，保留 no_linkage 依赖。26 项还与同 commit / 内容哈希的官方 API dependencies 对照；四项历史配方另外核对条件块。结果从 qemu 出发恰好覆盖 **30 个节点、39 条直接运行依赖引用**，没有额外 formula 或 openssl@4：

| 非叶子 formula | 本条件下的直接 Homebrew 运行依赖 |
| --- | --- |
| qemu | capstone, dtc, glib, gnutls, jpeg-turbo, libpng, libslirp, libssh, libusb, lzo, ncurses, pixman, snappy, vde, zstd |
| capstone | gettext |
| dtc | libyaml |
| gettext | json-c, libunistring |
| glib | gettext, pcre2 |
| gnutls | ca-certificates, gettext, gmp, libidn2, libtasn1, libunistring, nettle, p11-kit |
| libidn2 | gettext, libunistring |
| libslirp | glib |
| libssh | openssl@3 |
| nettle | gmp |
| openssl@3 | ca-certificates |
| p11-kit | ca-certificates, libtasn1 |
| zstd | lz4, xz |

其余 17 项在此条件下没有 Homebrew 运行依赖。`uses_from_macos` 单独保留为宿主输入：qemu / pcre2 使用 bzip2；gettext 使用 curl、libxml2、系统 ncurses；glib 使用 libffi、python；p11-kit 使用 libffi。它们不新增到 30 项 Homebrew 表，也不等于本轮验证了每个系统库或脚本的运行情况。源码构建需要的工具和 HEAD / Linux 专用资源不在本次 bottle 安装集合内；无法使用目标 bottle 时不能自动扩展到源码构建。

配方附属文本也已核对：glib 在来源 B 引用的 [Patches/glib/hardcoded-paths.diff](https://github.com/Homebrew/homebrew-core/blob/fc37e24fc3f15b020b7c5de326f10fd6c2ee0200/Patches/glib/hardcoded-paths.diff)为 1523 字节，SHA-256 `d846efd0bf62918350da94f850db33b0f8727fece9bfaf8164566e3094e80c97`；gnutls 的 [固定 commit 补丁](https://gitlab.com/gnutls/gnutls/-/commit/29c2027c963cf559817d0da37c1fc2efd0c1bd6a.diff)为 819 字节，SHA-256 `c0bfc0164ff131c9d2e1c4d0367430f8b767b64c9c21e0f408bd634d21fb0302`，与配方声明相同。snappy 的 DATA 补丁已包含在原文哈希内。glib 的 gobject-introspection 1.86.0 资源声明 SHA-256 为 `920d1a3fcedeadc32acff95c2e203b319039dd4b4a08dd1a2dfd283d19c0b9ae`；未下载归档。OpenSSL 的三个 CPAN 资源和 libpng 的 pngtest.png 都在 Linux 条件块内。本轮没有引入第三方源码到仓库。

静态读取发现四项 post_install_steps：glib 创建 Homebrew prefix 下的 `lib/gio/modules`；gnutls / openssl@3 更新各自 etc 中指向 ca-certificates 的 cert.pem 链接；ca-certificates 执行包内 post-install，读取系统证书链并写入 Homebrew etc 下的证书 bundle。这是宿主相关输出，不能仅用 bottle 哈希宣称证书集合可重复。本轮未执行这些动作，未读取系统钥匙串内容。

#### 固定集合与实际 Homebrew 事务的边界

本地调整记录目前为 **无**。若将来建立独立固定 tap，须逐项记录依赖全名改写、官方 bottle root_url 保留、附属补丁路径与改后文本哈希；不能把官方原文哈希当成本地修改版哈希，也不能仅复制 30 份未限定依赖名的配方就宣称停止了移动解析。

本机 Homebrew 源码提交 `b3625f73d3e3574c5789ee32eb7b06627b788ec4` 的 [formula_installer.rb](https://github.com/Homebrew/brew/blob/b3625f73d3e3574c5789ee32eb7b06627b788ec4/Library/Homebrew/formula_installer.rb)（SHA-256 `833fa39f2ab1e39aab4c772ec49e567a1420e32e54e84ec57e4649052b817fe5`）明确有两处不能忽略：

- 安装依赖时，已有 keg 收据的 tap 与候选 tap 不同会报错要求先卸载；相关分支在该检查前还包含 unlink / 临时改名操作。因此共享 `/opt/homebrew` 内改用同名独立 tap，可能触发与现有 core keg 的冲突，不能当作无副作用试运行，更不能自动卸载用户共享依赖来绕过。
- bottle 运行依赖记录按 full_name 建表，再按声明的 dep.name 查最小版本；把声明改为自定义 tap 全名后，原 core bottle 记录未必仍按同一个键匹配。因此固定引用仍须显式对照每项版本 / revision / 制品身份，不能依赖这个隐式最低版本检查保证冻结。

以上来自只读源码核对，没有触发安装事务。静态 AST 图证明候选之间的引用闭合；它不证明 tap 迁移、已装项复用、bottle 重定位或反向依赖安全。必须先形成兼容既有 keg 来源的具体实施方案并在相应授权范围核验，才可给出安装命令；不能改 Homebrew 本身、绕过依赖校验、伪造 core 身份或改写用户收据来获得表面通过。

## 3. 安装影响与清理边界

| 动作 | 写入与资源影响 | 恢复 / 清理方式 |
| --- | --- | --- |
| 安装指定 nightly 和组件 | 写入 `~/.rustup/toolchains/` 与下载缓存；可能占用数 GiB，实际以下载清单为准 | 保留现有 1.96.0 / 1.97.1 与默认设置；仅在确认未被其他项目使用时移除本任务新增工具链 |
| 安装固定 QEMU 集合 | 写入 `/opt/homebrew/Cellar`、opt / 链接目录、Homebrew 缓存及拟建本地固定 formula 集合；会触及共享依赖 | 安装前记录配方 / 收据 / opt 指向及旧版本；只处理本次资源，不做全局 autoremove / cleanup，不自动移除旧 keg |
| 构建 Linux 工具镜像 | OrbStack 镜像存储、构建缓存、包索引及专用输出目录产生写入 | 记录项目镜像 ID / digest 和容器 ID；退出本任务容器，仅清理归属明确的资源，不执行全局 prune |
| 后续制盘 / VM 验证 | 每份 raw 逻辑容量 1 GiB，副本数与实际占用另记；每次运行有独立 VARS 和日志；guest 基线 1 vCPU / 512 MiB，宿主另有仿真开销 | 仅使用仓库忽略目录内的任务普通文件，不挂宿主物理盘；结束关闭任务进程，保留失败副本时记录路径 |

选定的 30 项集合与本机 Cellar 比较后为 **22 项未安装、4 项版本升级、4 项同版本但制品身份待核对**。这取代移动 formula 图的 23 / 3 / 4 估计；它仍是静态差异，不是 Homebrew 安装事务或反向依赖重装清单；独立 tap 的来源冲突可能使此事务直接被拒绝，不能把 22 / 4 / 4 当成已可执行的动作数量。

| 共享依赖升级 | 本机 → 选定版本 | 本机收据中的使用者（非完整影响测试） |
| --- | --- | --- |
| pcre2 | 10.47_1 → 10.49 | Git 2.54.0、ripgrep 15.2.0 |
| snappy | 1.2.2 → 1.3.1 | leveldb 1.23_2、librime 1.17.0 |
| ca-certificates | 2026-08-13 → 2026-09-25 | OpenSSL 3、FFmpeg、Ruby、CocoaPods 的依赖记录 |
| openssl@3 | 3.6.4 → 3.6.4_1 | FFmpeg、Ruby、CocoaPods 的依赖记录；保留已有 3.6.2 / 3.6.4 的恢复证据 |

同版本四项为 gettext 1.0、libunistring 1.4.2、libyaml 0.2.5、xz 5.8.4。本机收据显示来自 macOS 26 / 26.6 构建，且没有直接提供与上表相同的 bottle rebuild / SHA 证明；不能直接记作“复用成功”。严格采用上表制品身份时，须先证明本机项匹配，否则列为需要重新安装的同版本项，并纳入授权和反向依赖核对；不能因版本相同跳过。

30 项全部制品的 registry 大小元数据合计下载 **166,586,895 字节**、安装大小 **930,446,628 字节**。这是整套候选的容量参考，含可能重装的四项，不是净新增占用，也不含缓存、旧 keg、本地配方、安装中间产物及额外反向依赖动作。其中 QEMU 本体为 113,262,802 / 728,988,917 字节（下载 / 安装）。均未下载或实测占盘。若必须改为源码构建，重新说明工具、版本、资源和时长，不沿用此估计。

时间仅作预算：Rust 与 QEMU 下载安装合计约 10–30 分钟，首次 Linux 工具镜像构建约 5–15 分钟，受网络与缓存影响，尚未测量。安装前列出包管理器实际解析的新增 / 升级项、总下载量与磁盘需求；不运行全局 upgrade，不改变 shell 配置或现有 Docker 默认 context。

Limine 二进制与 Rust crate 的配对、业务 crates / 功能开关、SDL 宿主 runner 依赖属于后续实现所需的单独候选，不包含在以上宿主工具安装集内。T01 最小 no_std 编译不需要先安装所有 T02–T05 依赖；形成引导镜像前必须完成 Limine 协议修订、资产哈希与许可核对。当前 raw / UEFI 方向不预装 xorriso、NASM、完整 LLVM 或 Linux Live 介质，确有实现需要时再补具体版本。

第三方项尚未引入仓库。Rust 工具链、QEMU、EDK II、Debian 基础系统与每个包分别按[许可与来源策略](../governance/licensing-and-sources.md)登记；QEMU formula 的 GPL-2.0-only 不覆盖所有附带固件，EDK II 另核对 `edk2-licenses.txt`。本表不是分发许可结论，也不承诺打包这些宿主工具。

## 4. 分层验证方案

以下均为后续执行方案，不能勾作本轮通过。安装完成与业务实现完成分开验收。

| 层次 | 方法与通过条件 | 失败处理 / 证据边界 |
| --- | --- | --- |
| 安装前身份核对 | Rust 日期 manifest 与组件可用；固定 formula 实际解析到第 2.2 节的 30 项版本 / revision / rebuild / bottle 哈希，解决 tap / keg 来源冲突；Debian 平台 digest、快照签名经实际 apt / sqv 策略验证，包身份及求解匹配 | 缺包、混入 OpenSSL 4、哈希不符、版本漂移或额外安装 / 重装项则停在该项；不自动换源或选 latest，先列出差异 |
| Rust 工具可运行 | 显式 `cargo +nightly-2026-10-13 -V` / `rustc +nightly-2026-10-13 -Vv`，核对已装 target / 组件及 rust-lld | 避免 `RUSTUP_TOOLCHAIN=1.96.0` 干扰；不改用户全局默认；版本输出仅证明工具可执行 |
| QEMU 功能枚举 | `qemu-system-x86_64 --version`、`-machine help`、`-accel help`、`-display help` 与 `qemu-img --version`；记录实际支持的精确 pc-q35-* 名称、TCG 和显示后端 | 当前没有独立 CLI，因此不编造最终机器版本；若缺 TCG 或所需后端，不降低为另一架构 / 加速器冒充通过 |
| 宿主链接与共享影响 | 安装授权后核对 QEMU / libssh 的 `otool -L`、递归库来源与链接检查，确认目标 OpenSSL 3；对涉及的 Git / ripgrep / leveldb / librime / FFmpeg / Ruby 等执行风险匹配的代表性验证 | 元数据 ABI 标记不能替代运行证据；不能全局屏蔽反向依赖检查来宣称安全；失败时保留旧 keg / 原 opt 指向及日志，报告影响后按明确范围恢复，不批量卸载 |
| 固件核对 | 读安装包 descriptor、许可证、两个文件尺寸与 SHA-256；代码只读，VARS 每次从模板复制 | 描述文件与实体路径不一致、架构不符则失败；不能用“有 .fd”证明可启动 |
| Linux 工具可运行 | 在项目专用 ARM64 容器读取 dpkg 精确包版本、sgdisk / mkfs.fat / mtools 版本；记录基础与最终镜像 digest、完整包清单 | 不进入其他项目容器；没有目标包不得报告环境就绪；不使用 privileged 或透传宿主设备 |
| 工具制镜像验证 | 只对新建普通文件生成规划中的 1 GiB / 512 字节扇区 GPT、128 MiB FAT32 ESP 和 DATA；检查主备 GPT、边界及 FAT；合成文件复制后用独立读取路径对照内容哈希 | 不把制盘工具成功当作内核持久化通过；读盘前确保无活动 guest，不自动修复损坏样本 |
| T01 工程验证 | 实现获授权后建立真实 Cargo workspace 和入口；干净输出目录编译 x86_64-unknown-none 最小 no_std 内核，确认 ELF x86_64；宿主与内核依赖图隔离，记录实际 features / lockfile | 不能用宿主 cargo check 或版本输出替代裸机编译；无入口时不建占位成功检查 |
| T02–T05 / M0 | 固定精确机器名、CPU / 设备参数、TCG、固件及镜像哈希；取得引导、写入、flush、重启读回、宿主独立校验与失败证据 | 依 MVP 的 T / AT 执行；UEFI 启动画面、最小编译或模拟器通过均不能替代 M0 完成 |

ARM64 宿主模拟 x86_64 使用 TCG，硬件虚拟化要求宿主与 guest 架构匹配；不以 ARM64 容器可运行推断 x86_64 guest 已验证。[UTM 架构说明](https://docs.getutm.app/settings-qemu/system/)、[QEMU 加速器与能力枚举](https://www.qemu.org/docs/master/system/introduction.html)

## 5. 盘点限制与复核入口

本机事实来自 `uname` / `sw_vers`、`command -v`、rustup installed 查询、已有 rustc / cargo / rust-lld 的版本输出、应用 Info.plist、`file` / `strings`、固件 descriptor / `shasum` / 文件尺寸，以及 Docker 格式化只读 API。新增核对包括 Docker / GHCR registry JSON、固定官方配方与 AST、签名快照索引、包路径 HEAD 与本地安装收据。UTM 动态库字符串不是独立 QEMU 的运行结果；本机性能、nightly 实际编译、完整 formula 求解 / 动态链接及所有产品行为仍未验证。

Homebrew 的 `list --versions` 即使设置 NO_AUTO_UPDATE 仍尝试刷新 API，访问代理失败；其失败不能算安装清单查询成功，后续改为只读 Cellar。Docker API 首次被沙盒拒绝，经同一只读目标的权限批准后返回服务版本与架构。sysctl 硬件查询被权限限制，未据此填入 CPU 型号或内存容量。以上都没有启动服务或安装依赖。

本机 Homebrew 源码显示 `install --dry-run` 仍会经过 `perform_preinstall_checks` → `attempt_directory_creation`，可能创建缺失目录；本轮未运行它，采用元数据和收据比较。首次依赖 registry 查询误把 rebuild 拼为 `.N` 导致 404；按本机 `GitHubPackages.version_rebuild` 规则改为 index tag 的 `-N` 后，所选依赖元数据均取得，不能把先前 404 解释为包不存在。
