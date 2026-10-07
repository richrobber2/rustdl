#!/usr/bin/env python3
"""Require added release notes in the committed range being pushed."""
import argparse
import json
import re
import subprocess
import sys

CHANGELOG = 'src/local/pages/changelog.rs'
ZERO = re.compile(r'^0+$')


def git(*args):
    return subprocess.check_output(['git', *args], text=True, input='').strip()


def notes(revision):
    if not revision or ZERO.fullmatch(revision):
        return set()
    result = subprocess.run(['git', 'show', f'{revision}:{CHANGELOG}'], text=True, capture_output=True)
    if result.returncode:
        return set()
    text = result.stdout
    match = re.search(r'const CHANGELOG:[^=]*=\s*&\[(.*?)\n\];', text, re.S)
    if not match:
        return set()
    values = re.findall(r'"(?:[^"\\]|\\.)*"', match[1])
    decoded = [json.loads(value).strip() for value in values]
    return {
        value for value in decoded
        if len(value) >= 12 and len(value.split()) >= 3
        and not re.match(r'(?i)^(TODO|TBD|FIXME|placeholder)\b', value)
    }


def check(base, head):
    if not base or ZERO.fullmatch(base):
        base = git('hash-object', '-t', 'tree', '--stdin')
    changed = git('diff', '--name-only', base, head).splitlines()
    if not changed:
        return
    if not (notes(head) - notes(base)):
        raise ValueError(f'{head[:12]} changes content without adding release notes to {CHANGELOG}. Add concrete notes, commit them, and push again.')


def pre_push(remote, updates):
    for line in updates:
        fields = line.split()
        if len(fields) != 4:
            raise ValueError('Invalid pre-push input; refusing an unchecked push.')
        local_ref, head, remote_ref, base = fields
        if ZERO.fullmatch(head):
            continue  # Deleting a ref does not introduce changed content.
        if ZERO.fullmatch(base):
            commits = git('rev-list', '--reverse', head, '--not', f'--remotes={remote}').splitlines()
            if not commits:
                continue  # Existing remote history under a new branch name.
            parents = git('rev-list', '--parents', '-n', '1', commits[0]).split()
            base = parents[1] if len(parents) > 1 else None
        check(base, head)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--range', nargs=2, metavar=('BASE', 'HEAD'))
    parser.add_argument('--pre-push', metavar='REMOTE')
    args = parser.parse_args()
    try:
        if args.range:
            check(*args.range)
        elif args.pre_push:
            pre_push(args.pre_push, sys.stdin)
        else:
            parser.error('Provide --range BASE HEAD or --pre-push REMOTE')
    except (ValueError, subprocess.CalledProcessError) as error:
        print(f'Changelog guard: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
