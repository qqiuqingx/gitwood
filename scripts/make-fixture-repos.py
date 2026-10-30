#!/usr/bin/env python3
# Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
"""Generate an isolated 50,001-commit/200-merge/10,001-file repository via fast-import.

All feature branches fork from the initial root. Main advances 149 commits between
merges; each side branch has 100 commits. This deliberately leaves many long edges
to the shared root. Merge trees inherit main; these are synthetic graph fixtures,
not a merge-conflict correctness benchmark. No fixture is stored in the source repo.
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    destination = Path(sys.argv[1] if len(sys.argv) > 1 else
                       Path(tempfile.gettempdir()) / 'gitwood-fixtures').resolve()
    source_root = Path(__file__).resolve().parent.parent
    if destination == source_root or source_root in destination.parents:
        raise SystemExit('Fixture output must be outside the source repository')
    repository = destination / 'graph-50k'
    if repository.exists():
        raise SystemExit(f'Refusing to overwrite existing path: {repository}')
    repository.mkdir(parents=True)
    env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull)
    subprocess.run(['git', 'init', '--quiet', '--initial-branch=main', '--template=',
                    str(repository)], env=env, check=True)
    process = subprocess.Popen(['git', '-C', str(repository), 'fast-import', '--quiet'],
                               stdin=subprocess.PIPE, env=env)
    stream = process.stdin
    next_mark = 0
    commits = 0

    def send(text):
        stream.write(text.encode('utf-8'))

    def blob(content):
        nonlocal next_mark
        next_mark += 1
        send(f'blob\nmark :{next_mark}\ndata {len(content.encode())}\n{content}\n')
        return next_mark

    shared_blob = blob('Gitwood static fixture file\n')

    def commit(ref, parent=None, merge=None, initial=False):
        nonlocal commits, next_mark
        data_mark = blob(f'fixture revision {commits}\n')
        next_mark += 1
        commit_mark = next_mark
        commits += 1
        message = f'fixture commit {commits}\n'
        send(f'commit refs/heads/{ref}\nmark :{commit_mark}\n'
             f'committer Gitwood Fixture <fixture@example.invalid> {1700000000 + commits} +0000\n'
             f'data {len(message.encode())}\n{message}')
        if parent is not None:
            send(f'from :{parent}\n')
        if merge is not None:
            send(f'merge :{merge}\n')
        if initial:
            for index in range(10_000):
                send(f'M 100644 :{shared_blob} files/{index:05}.txt\n')
        send(f'M 100644 :{data_mark} progress.txt\n\n')
        return commit_mark

    try:
        root = commit('main', initial=True)
        main_tip = root
        for branch in range(200):
            feature_tip = root
            for _ in range(100):
                feature_tip = commit(f'feature-{branch:03}', parent=feature_tip)
            for _ in range(149):
                main_tip = commit('main', parent=main_tip)
            main_tip = commit('main', parent=main_tip, merge=feature_tip)
        send('done\n')
        stream.close()
        if process.wait() != 0:
            raise SystemExit('git fast-import failed; fixture retained for inspection')
    except BaseException:
        stream.close()
        process.wait()
        raise
    subprocess.run(['git', '-C', str(repository), 'reset', '--hard', '--quiet', 'main'],
                   env=env, check=True)
    metadata = {'repository': str(repository), 'commits': commits, 'feature_branches': 200,
                'merges': 200, 'files_at_main': 10001, 'generator': 'git fast-import',
                'git_version': subprocess.check_output(['git', '--version'], text=True).strip()}
    (destination / 'graph-50k.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(json.dumps(metadata, indent=2))


if __name__ == '__main__':
    main()
