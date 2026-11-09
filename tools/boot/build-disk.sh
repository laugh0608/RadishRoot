#!/bin/sh
# 只由 xtask 在固定工具容器内调用；数值由 manifest 校验后按位置传入。
set -eu
export LC_ALL=C.UTF-8 MTOOLSRC=/dev/null
cd /output
test ! -e disk.raw
bytes=$1
esp_start=$2
esp_sectors=$3
data_start=$4
last_lba=$5
truncate -s "$bytes" disk.raw
sgdisk --clear --disk-guid=11111111-2222-4333-8444-555555555555 \
  --new="1:$esp_start:$((data_start - 1))" --typecode=1:ef00 --change-name=1:ESP \
  --partition-guid=1:aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee \
  --new="2:$data_start:$last_lba" --typecode=2:0700 --change-name=2:DATA \
  --partition-guid=2:12345678-1234-4234-8234-123456789abc disk.raw
truncate -s "$((esp_sectors * 512))" esp.fat
truncate -s "$(((last_lba - data_start + 1) * 512))" data.fat
mkfs.fat -F 32 -S 512 -i 52524553 -n ESP esp.fat
mkfs.fat -F 32 -S 512 -i 52524441 -n DATA data.fat
mmd -i esp.fat ::/EFI ::/EFI/BOOT ::/licenses
mcopy -i esp.fat /inputs/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mcopy -i esp.fat /inputs/kernel.elf ::/kernel.elf
mcopy -i esp.fat /inputs/limine.conf ::/limine.conf
mcopy -i esp.fat /inputs/LICENSE ::/licenses/LICENSE
mcopy -i esp.fat /inputs/3RDPARTY.md ::/licenses/3RDPARTY.md
for license in /inputs/crate-licenses/*.txt; do
  mcopy -i esp.fat "$license" "::/licenses/$(basename "$license")"
done
fsck.fat -n esp.fat
fsck.fat -n data.fat
dd if=esp.fat of=disk.raw bs=512 seek="$esp_start" conv=notrunc,sparse status=none
dd if=data.fat of=disk.raw bs=512 seek="$data_start" conv=notrunc,sparse status=none
sgdisk --verify disk.raw
# 独立于写入源重新读回磁盘上的文件，禁止只核对暂存 FAT。
mcopy -i "disk.raw@@$((esp_start * 512))" ::/kernel.elf kernel-readback.elf
mcopy -i "disk.raw@@$((esp_start * 512))" ::/EFI/BOOT/BOOTX64.EFI boot-readback.efi
mcopy -i "disk.raw@@$((esp_start * 512))" ::/limine.conf config-readback.conf
cmp /inputs/kernel.elf kernel-readback.elf
cmp /inputs/BOOTX64.EFI boot-readback.efi
cmp /inputs/limine.conf config-readback.conf
sha256sum disk.raw
