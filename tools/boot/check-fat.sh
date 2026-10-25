#!/bin/sh
# xtask 唯一编排入口；普通文件磁盘由本次任务创建。guest 退出后才允许调用。
set -eu
export LC_ALL=C.UTF-8 MTOOLSRC=/dev/null
mode=$1
offset=$2
length=$3
disk="/disk/disk.raw@@$offset"
cd /output
dpkg-query -W -f='${Package}\t${Version}\n' mtools dosfstools
case "$mode" in
  prepare)
    mcopy -i "$disk" /inputs/seed.bin ::/seed.txt
    ;;
  verify|observe|observe-file)
    mdir -b -s -i "$disk" ::/ > paths.txt
    while IFS="$(printf '\t')" read -r id name; do
        test ! -e "$id.bin"
        mcopy -i "$disk" "::/$name" "$id.bin"
        cmp "/inputs/$id.bin" "$id.bin"
    done < /inputs/files.tsv
    if [ "$mode" = observe-file ]; then
      mcopy -i "$disk" ::/failed.txt failed.bin
      test ! -s failed.bin
    fi
    test ! -e data.fat
    dd if=/disk/disk.raw of=data.fat bs=4M skip="$offset" count="$length" \
      iflag=skip_bytes,count_bytes conv=sparse status=none
    set +e
    fsck.fat -n data.fat > fsck.txt 2>&1
    result=$?
    set -e
    printf '%s\n' "$result" > fsck-exit.txt
    if [ "$mode" = verify ]; then test "$result" = 0; fi
    ;;
  *) exit 2 ;;
esac
