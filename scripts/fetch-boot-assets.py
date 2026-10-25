#!/usr/bin/env python3
"""校验 T02 引导资产；只有显式 --download 才允许下载固定归档。"""
import argparse
import hashlib
from pathlib import Path
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--download', action='store_true')
    args = parser.parse_args()
    manifest = dict(line.split('\t', 1) for line in (ROOT / 'tools/boot/manifest.tsv').read_text().splitlines()
                    if line and not line.startswith('#'))
    out = ROOT / 'work/t02-assets'
    out.mkdir(parents=True, exist_ok=True)
    if out.resolve() != out or (ROOT / 'work').is_symlink():
        raise ValueError('asset output must not traverse symlinks')
    archive = out / f'limine-binary-{manifest["limine_version"]}.tar.xz'
    if archive.is_symlink():
        raise ValueError('archive must not be a symlink')
    if not archive.exists():
        if not args.download:
            raise ValueError('missing archive; explicit authorized --download required')
        request = urllib.request.Request(manifest['limine_url'], headers={'User-Agent': 'RadishRoot-T02'})
        with urllib.request.urlopen(request, timeout=60) as response:
            data = response.read(8 * 1024 * 1024 + 1)
        if len(data) > 8 * 1024 * 1024:
            raise ValueError('archive exceeds limit')
        if hashlib.sha256(data).hexdigest() != manifest['limine_archive_sha256']:
            raise ValueError('download checksum mismatch')
        with archive.open('xb') as file:
            file.write(data)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != manifest['limine_archive_sha256']:
        raise ValueError('cached archive checksum mismatch; not replaced')
    names = {'BOOTX64.EFI': 'efi', 'LICENSE': 'license', '3RDPARTY.md': 'notices'}
    with tarfile.open(archive) as tar:
        for name, key in names.items():
            member = tar.getmember('limine-binary/' + name)
            if not member.isfile() or member.size > 2 * 1024 * 1024:
                raise ValueError('unexpected archive entry')
            data = tar.extractfile(member).read()
            if hashlib.sha256(data).hexdigest() != manifest[f'limine_{key}_sha256']:
                raise ValueError('asset checksum mismatch: ' + name)
            target = out / name
            if target.is_symlink():
                raise ValueError('asset path is a symlink')
            if target.exists():
                if target.read_bytes() != data:
                    raise ValueError('existing asset differs; not replaced: ' + name)
            else:
                with target.open('xb') as file:
                    file.write(data)
            print(f'verified {name}: {len(data)} bytes')


if __name__ == '__main__':
    main()
