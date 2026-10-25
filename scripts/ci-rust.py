#!/usr/bin/env python3
"""CI Rust bootstrap and checks; toolchain identity comes only from repository TOML."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('install', 'verify'))
    args = parser.parse_args()
    config = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']
    channel = config['channel']
    if args.action == 'install':
        # Explicit CI-only installation step; never run as part of local xtask.
        if os.environ.get('GITHUB_ACTIONS') != 'true':
            parser.error('install is restricted to the explicitly configured GitHub Actions step')
        subprocess.run(['rustup', 'toolchain', 'install', channel, '--profile', config['profile'],
                        '--component', ','.join(config['components']),
                        '--target', ','.join(config['targets']), '--no-self-update'],
                       cwd=ROOT, check=True, timeout=600)
    else:
        environment = dict(os.environ, RUSTUP_AUTO_INSTALL='0')
        for command in [('check',), ('test', '--host'),
                        ('build', '--profile', 'dev', '--fresh'),
                        ('build', '--profile', 'release', '--fresh')]:
            subprocess.run(['cargo', f'+{channel}', 'xtask', *command], cwd=ROOT,
                           env=environment, check=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode if error.returncode > 0 else 1)
    except subprocess.TimeoutExpired:
        sys.exit(124)
