#!/bin/sh
# 仅由 Dockerfile 在固定 Debian ARM64 构建容器中执行。
set -eu
export LC_ALL=C

sources=/usr/local/share/radishroot-debian-tools/build
evidence=/usr/local/share/radishroot-debian-tools
scratch=/tmp/radishroot-debian-build
mkdir "$scratch"
cd "$scratch"
test "$(dpkg --print-architecture)" = arm64
dpkg-query -W -f='${binary:Package}\t${Version}\n' > base.actual.tsv
cmp "$sources/base-packages.tsv" base.actual.tsv
test "$(wc -l < "$sources/packages.tsv")" -eq 5
test ! -e /etc/apt/sources.list
test "$(find /etc/apt/sources.list.d -type f | wc -l)" -eq 1
cmp "$sources/snapshot.sources" /etc/apt/sources.list.d/debian.sources

mkdir empty.conf.d empty.sources.d lists archives lists/partial archives/partial
: > empty.conf
chown _apt:root lists/partial archives/partial
cat > apt.conf <<'CONFIG'
Dir::Etc::parts "/tmp/radishroot-debian-build/empty.conf.d";
Dir::Etc::main "/tmp/radishroot-debian-build/empty.conf";
Dir::Etc::sourcelist "/etc/apt/sources.list.d/debian.sources";
Dir::Etc::sourceparts "/tmp/radishroot-debian-build/empty.sources.d";
Dir::State::lists "/tmp/radishroot-debian-build/lists";
Dir::State::status "/var/lib/dpkg/status";
Dir::Cache::archives "/tmp/radishroot-debian-build/archives";
Dir::Cache::pkgcache "";
Dir::Cache::srcpkgcache "";
Acquire::Languages "none";
Acquire::PDiffs "false";
Acquire::Retries "0";
Acquire::http::Timeout "30";
Acquire::Check-Date "true";
Acquire::Check-Valid-Until "true";
Acquire::AllowInsecureRepositories "false";
Acquire::AllowDowngradeToInsecureRepositories "false";
APT::Get::AllowUnauthenticated "false";
APT::Update::Error-Mode "any";
CONFIG
export APT_CONFIG="$scratch/apt.conf"
apt-config dump > "$evidence/apt-config.txt"
apt-get update > "$evidence/apt-update.log" 2>&1 || {
    cat "$evidence/apt-update.log"; exit 1;
}
cat "$evidence/apt-update.log"
if grep -E '^(W:|E:)|NO_PUBKEY|unsandboxed' "$evidence/apt-update.log"; then
    echo 'APT 更新未通过完整验证。' >&2; exit 1
fi

set --
while IFS="$(printf '\t')" read -r package version arch size sha; do
    test "$arch" = arm64
    set -- "$@" "$package=$version"
done < "$sources/packages.tsv"
apt-get --yes --download-only --no-install-recommends --no-remove install "$@"
test "$(find archives -maxdepth 1 -type f -name '*.deb' | wc -l)" -eq 5
while IFS="$(printf '\t')" read -r package version arch size sha; do
    archive="archives/${package}_${version}_${arch}.deb"
    test "$(wc -c < "$archive")" -eq "$size"
    printf '%s  %s\n' "$sha" "$archive" | sha256sum -c -
    test "$(dpkg-deb -f "$archive" Package)" = "$package"
    test "$(dpkg-deb -f "$archive" Version)" = "$version"
    test "$(dpkg-deb -f "$archive" Architecture)" = "$arch"
done < "$sources/packages.tsv"
(cd archives && sha256sum ./*.deb) > "$evidence/package-bodies.sha256"

# 所有包体核对通过后才安装；缺包时禁止下载其他包补齐。
DEBIAN_FRONTEND=noninteractive apt-get --yes --no-download --no-install-recommends --no-remove install "$@" > "$evidence/apt-install.log" 2>&1 || {
    cat "$evidence/apt-install.log"; exit 1;
}
cat "$evidence/apt-install.log"
dpkg --audit > "$evidence/dpkg-audit.txt"
test ! -s "$evidence/dpkg-audit.txt"
dpkg-query -W -f='${binary:Package}\t${Version}\n' > "$evidence/packages.tsv"
test "$(wc -l < "$evidence/packages.tsv")" -eq 83
cp "$evidence/packages.tsv" base.retained.tsv
while IFS="$(printf '\t')" read -r package version arch size sha; do
    actual=$(dpkg-query -W -f='${Version}|${Architecture}|${Status}' "$package")
    test "$actual" = "$version|$arch|install ok installed"
    awk -F '\t' -v package="$package" '{split($1, name, ":"); if (name[1] != package) print}' base.retained.tsv > base.next.tsv
    mv base.next.tsv base.retained.tsv
done < "$sources/packages.tsv"
cmp "$sources/base-packages.tsv" base.retained.tsv
(cd "$sources" && sha256sum Dockerfile build-image.sh packages.tsv base-packages.tsv snapshot.sources) > "$evidence/recipe.sha256"
sha256sum /var/lib/dpkg/status > "$evidence/dpkg-status.sha256"
printf '包体校验通过：5；已安装包：83；原有 78 包版本保持不变。\n'

# 只删除此构建阶段创建的缓存；包版权文件和构建证据保留在镜像内。
cd /
rm -rf /tmp/radishroot-debian-build
