#!/usr/bin/env python3
"""RadishRoot repository checks; standard library only, no network or writes."""
import argparse
import fnmatch
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = (
    'AGENTS.md', 'CLAUDE.md', 'README.md', 'CONTRIBUTING.md', 'SECURITY.md',
    'CODE_OF_CONDUCT.md', '.editorconfig', '.gitattributes', '.gitignore',
    'docs/README.md', 'docs/status/current.md', 'docs/product-plan.md',
    'docs/mvp-plan.md', 'docs/development/standards.md',
    'docs/governance/agent-collaboration.md',
    'docs/governance/repository-governance.md',
    'docs/governance/licensing-and-sources.md',
    'docs/adr/0002-repository-foundation.md',
    '.github/PULL_REQUEST_TEMPLATE.md', '.github/ISSUE_TEMPLATE/config.yml',
    '.github/ISSUE_TEMPLATE/bug-report.md', '.github/ISSUE_TEMPLATE/proposal.md',
    '.github/rulesets/master-protection.json', '.github/rulesets/README.md',
    '.github/workflows/pr-check.yml', 'scripts/check-repo.py',
    'scripts/tests/test_check_repo.py', 'scripts/ci-rust.py',
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml',
    'kernel/Cargo.toml', 'kernel/src/main.rs', 'kernel/linker.ld', 'kernel/build.rs',
    'xtask/Cargo.toml', 'xtask/src/main.rs', 'xtask/build.rs',
)
BINARY = {'.png', '.jpg', '.jpeg', '.gif', '.webp', '.ico', '.pdf', '.woff2', '.zip'}
COMMIT = re.compile(r'^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([\w./-]+\))?!?: \S.*$')


def git(root, *args):
    result = subprocess.run(['git', '-C', str(root), *args], capture_output=True)
    if result.returncode:
        detail = (result.stderr + result.stdout).decode(errors='replace').strip()
        raise ValueError(f'git {args[0]} failed: {detail}')
    return result.stdout


def ignored(name, patterns):
    """File-mode subset: slash-rooted paths, component globs, directories, !."""
    result = False
    for pattern in patterns:
        negate = pattern.startswith('!')
        pattern = pattern.lstrip('!')
        anchored = pattern.startswith('/')
        directory = pattern.endswith('/')
        pattern = pattern.strip('/')
        parts = name.split('/')
        if anchored or '/' in pattern:
            matched = fnmatch.fnmatchcase(name, pattern)
            if directory:
                matched = name.startswith(pattern + '/')
        else:
            matched = any(fnmatch.fnmatchcase(p, pattern) for p in (parts[:-1] if directory else parts))
        if matched:
            result = not negate
    return result


def inventory(root):
    patterns = [s.strip() for s in (root / '.gitignore').read_text().splitlines()
                if s.strip() and not s.lstrip().startswith('#')]
    has_git = (root / '.git').exists()
    if has_git:
        tracked = set(git(root, 'ls-files', '-z').decode().split('\0')) - {''}
        untracked = set(git(root, 'ls-files', '--others', '--exclude-standard', '-z').decode().split('\0')) - {''}
        # Deleted tracked files are assessed through required paths / links and diff.
        files = sorted(p for p in tracked | untracked if (root / p).exists() or (root / p).is_symlink())
        forbidden = sorted(p for p in tracked if ignored(p, patterns))
        return files, has_git, forbidden
    files = []
    for parent, dirs, names in os.walk(root, followlinks=False):
        rel = Path(parent).relative_to(root)
        dirs[:] = [d for d in dirs if d != '.git' and not ignored((rel / d / '_').as_posix(), patterns)]
        for d in dirs[:]:
            if (Path(parent) / d).is_symlink():
                files.append((rel / d).as_posix())
                dirs.remove(d)
        files.extend((rel / n).as_posix() for n in names if not ignored((rel / n).as_posix(), patterns))
    return sorted(files), has_git, []


def check_links(root, name, text):
    errors = []
    # Ignore fenced examples. Inline and reference links are supported, anchors are not validated.
    text = re.sub(r'^(`{3,}|~{3,}).*?^\1[^\n]*$', '', text, flags=re.M | re.S)
    links = re.findall(r'\]\(\s*(<[^>]+>|[^\s)]+)', text)
    links += re.findall(r'^\s*\[[^\]]+\]:\s*(<[^>]+>|\S+)', text, flags=re.M)
    for target in links:
        target = target.strip('<>')
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc or not parsed.path:
            continue
        candidate = (root / name).parent / unquote(parsed.path)
        resolved = candidate.resolve()
        if not resolved.is_relative_to(root.resolve()):
            errors.append(f'{name}: link leaves repository: {target}')
        elif not resolved.exists():
            errors.append(f'{name}: missing link: {target}')
    return errors


def check_contract(root):
    errors = []
    def need(condition, message):
        if not condition:
            errors.append(message)
    ruleset = json.loads((root / '.github/rulesets/master-protection.json').read_text())
    need(ruleset['target'] == 'branch' and ruleset['enforcement'] == 'active', 'ruleset target/enforcement drift')
    need(ruleset['conditions'] == {'ref_name': {'include': ['refs/heads/master'], 'exclude': []}}, 'ruleset branch scope drift')
    need(ruleset['bypass_actors'] == [{'actor_id': 5, 'actor_type': 'RepositoryRole', 'bypass_mode': 'pull_request'}], 'ruleset bypass drift')
    rules = {r['type']: r for r in ruleset['rules']}
    need(len(ruleset['rules']) == 4 and set(rules) == {'deletion', 'non_fast_forward', 'pull_request', 'required_status_checks'}, 'ruleset rules drift')
    pr = rules['pull_request']['parameters']
    need(pr == {'allowed_merge_methods': ['merge', 'rebase'], 'dismiss_stale_reviews_on_push': True,
                'require_code_owner_review': False, 'require_last_push_approval': False,
                'required_approving_review_count': 0, 'required_review_thread_resolution': True}, 'ruleset PR contract drift')
    status = rules['required_status_checks']['parameters']
    need(status['strict_required_status_checks_policy'] is True and status['do_not_enforce_on_create'] is False, 'ruleset strict/create drift')
    checks = status['required_status_checks']
    need(len(checks) == 1 and checks[0]['context'] == 'Candidate Quality', 'required context drift')
    for check in checks:
        need(set(check) <= {'context', 'integration_id'}, 'unknown status-check field')
        if 'integration_id' in check:
            need(type(check['integration_id']) is int and check['integration_id'] > 0, 'invalid integration_id')
    workflow = json.loads((root / '.github/workflows/pr-check.yml').read_text())
    need(workflow['on'] == {'pull_request': {'branches': ['dev', 'master']}, 'workflow_dispatch': {}}, 'workflow triggers drift')
    need(workflow['permissions'] == {'contents': 'read'}, 'workflow permissions drift')
    jobs = workflow['jobs']
    need(set(jobs) == {'repo-hygiene', 'rust-checks', 'candidate-quality'}, 'workflow components drift: update declared baseline together')
    gate = jobs['candidate-quality']
    need(gate['name'] == 'Candidate Quality' and gate['if'] == '${{ always() }}', 'aggregate name/condition drift')
    need(gate['needs'] == ['repo-hygiene', 'rust-checks'], 'aggregate dependencies drift')
    step = gate['steps'][0]
    need(len(gate['steps']) == 1 and step['env'] == {'NEEDS_JSON': '${{ toJSON(needs) }}'}, 'aggregate result binding drift')
    need('set(needs) == {"repo-hygiene", "rust-checks"} and all(job.get("result") == "success" for job in needs.values())' in step['run'], 'aggregate success-only contract drift')
    component = jobs['repo-hygiene']
    need(component['name'] == 'Repo Hygiene', 'component name drift')
    checkout, check, tests = component['steps']
    need(checkout['uses'] == 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683', 'checkout pin drift')
    need(checkout['with'] == {'fetch-depth': 0, 'persist-credentials': False}, 'candidate checkout/credentials drift')
    need(check['env'] == {'BASE_SHA': '${{ github.event.pull_request.base.sha }}'}, 'PR base binding drift')
    need(check['run'] == 'if [[ -n "$BASE_SHA" ]]; then\n  python3 scripts/check-repo.py --base-ref "$BASE_SHA"\nelse\n  python3 scripts/check-repo.py\nfi', 'check command drift')
    need(tests['run'] == 'python3 -B -m unittest discover -s scripts/tests -v', 'regression command drift')
    rust = jobs['rust-checks']
    need(rust['name'] == 'Rust Checks' and 'if' not in rust, 'Rust component name/condition drift')
    need(rust['steps'] == [checkout,
                          {'name': 'Install repository toolchain', 'run': 'python3 scripts/ci-rust.py install'},
                          {'name': 'Check, test and link both profiles', 'run': 'python3 scripts/ci-rust.py verify'}],
         'Rust check commands/checkout drift')
    timeouts = {'repo-hygiene': 5, 'rust-checks': 15, 'candidate-quality': 2}
    for name, job in jobs.items():
        need(job['runs-on'] == 'ubuntu-24.04', 'runner drift')
        need(type(job['timeout-minutes']) is int and job['timeout-minutes'] == timeouts.get(name), 'job timeout missing/invalid')
        need(not any(k in job for k in ('continue-on-error', 'permissions', 'environment')), 'job bypass/permissions/deployment drift')
        for step in job['steps']:
            need(not any(k in step for k in ('if', 'continue-on-error')), 'step can skip/ignore failure')
    need('if' not in component, 'component can be skipped')
    need(json.loads((root / '.github/ISSUE_TEMPLATE/config.yml').read_text()) == {'blank_issues_enabled': False}, 'issue routing drift')
    attributes = (root / '.gitattributes').read_text().splitlines()
    need('* text=auto eol=lf' in attributes and '*.png binary' in attributes, 'Git text/binary policy drift')
    editor = (root / '.editorconfig').read_text()
    need(all(line in editor.splitlines() for line in ('root = true', 'charset = utf-8', 'end_of_line = lf',
                                                     'insert_final_newline = true', 'trim_trailing_whitespace = true')),
         'editor text policy drift')
    patterns = [line.strip() for line in (root / '.gitignore').read_text().splitlines()
                if line.strip() and not line.startswith('#')]
    need(all(ignored(name, patterns) for name in ('.env', 'target/cache', 'runtime/disk.img', 'disk.qcow2', 'output.iso')),
         'generated/private ignore policy drift')
    need(not any(ignored(name, patterns) for name in ('Cargo.lock', '.env.example', 'docs/assets/ui-layout.png')),
         'source/lockfile incorrectly ignored')
    return errors


def check_repository(root, base_ref=None):
    errors = []
    for name in REQUIRED:
        path = root / name
        if any(part.is_symlink() for part in (path, *path.parents) if part.is_relative_to(root)):
            errors.append(f'required path must not be a symlink: {name}')
        elif not path.is_file():
            errors.append(f'missing required file: {name}')
    if errors:
        return errors, 'incomplete', 0
    files, has_git, forbidden = inventory(root)
    errors.extend(f'generated/private file is tracked: {p}' for p in forbidden)
    for name in files:
        path = root / name
        if path.is_symlink():
            errors.append(f'{name}: symlinks require explicit policy; not followed')
            continue
        if len(name) > 180 or path.stat().st_size > 10 * 1024 * 1024:
            errors.append(f'{name}: path or file size limit exceeded')
            continue
        if path.suffix.lower() in BINARY:
            continue
        try:
            data = path.read_bytes()
            text = data.decode('utf-8')
        except UnicodeError:
            errors.append(f'{name}: not UTF-8; register binary type if intentional')
            continue
        if data.startswith(b'\xef\xbb\xbf') or b'\r' in data or b'\0' in data or (data and not data.endswith(b'\n')):
            errors.append(f'{name}: BOM/CR/NUL/missing final newline')
        if any(line.rstrip(' \t') != line for line in text.splitlines()):
            errors.append(f'{name}: trailing whitespace')
        if path.suffix in {'.json', '.yml', '.yaml'}:
            try:
                json.loads(text)
            except ValueError as exc:
                errors.append(f'{name}: expected JSON-compatible YAML/JSON: {exc}')
        if path.suffix == '.md':
            errors.extend(check_links(root, name, text))
    if (root / 'AGENTS.md').read_bytes() != (root / 'CLAUDE.md').read_bytes():
        errors.append('AGENTS.md / CLAUDE.md differ')
    try:
        errors.extend(check_contract(root))
    except (KeyError, ValueError, TypeError, IndexError) as exc:
        errors.append(f'invalid governance contract: {exc}')
    if has_git:
        for args in (('diff', '--check'), ('diff', '--cached', '--check')):
            try:
                git(root, *args)
            except ValueError as exc:
                errors.append(str(exc))
    if base_ref:
        if not has_git:
            errors.append('--base-ref requires a Git repository')
        else:
            try:
                # Resolve to a SHA before range interpolation; reject option-like input.
                base = git(root, 'rev-parse', '--verify', '--end-of-options', base_ref + '^{commit}').decode().strip()
                ancestor = git(root, 'merge-base', base, 'HEAD').decode().strip()
                git(root, 'diff', '--check', ancestor, 'HEAD')
                commits = git(root, 'log', '--format=%H%x09%P%x09%s', f'{ancestor}..HEAD').decode()
                for entry in commits.splitlines():
                    sha, parents, subject = entry.split('\t', 2)
                    if len(parents.split()) <= 1 and not COMMIT.fullmatch(subject):
                        errors.append(f'{sha[:12]}: not a Conventional Commit: {subject}')
            except ValueError as exc:
                errors.append(str(exc))
    return errors, 'Git worktree' if has_git else 'files only; Git/history/branches not checked', len(files)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-ref', help='PR base ref; requires Git history')
    args = parser.parse_args()
    if sys.version_info < (3, 10):
        parser.error('Python 3.10+ required')
    try:
        errors, mode, count = check_repository(ROOT, args.base_ref)
    except (OSError, ValueError) as exc:
        print(f'ERROR: {exc}', file=sys.stderr)
        return 1
    print(f'Mode: {mode}; files checked: {count}')
    for error in errors:
        print(f'ERROR: {error}', file=sys.stderr)
    print('Repository checks FAILED' if errors else 'Repository checks passed (governance only)')
    return 1 if errors else 0


if __name__ == '__main__':
    sys.exit(main())
