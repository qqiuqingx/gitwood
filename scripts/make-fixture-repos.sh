#!/bin/sh
# Copyright 2026 Gitwood contributors. SPDX-License-Identifier: Apache-2.0
# Requires Python 3 and Git. Refuses to overwrite an existing fixture repository.
set -eu
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
exec python3 "$script_dir/make-fixture-repos.py" "${1:-/tmp/gitwood-fixtures}"
