#!/usr/bin/env bash
# Turn Esdieff's coloured terminal output into an HTML block for the documentation.
#
#   { echo '$ esdieff check thermostat.sdf.json'; esdieff --color=always check thermostat.sdf.json; } \
#     | scripts/ansi-to-html.sh
#
# The palette is the one the command-line tool uses: purple headings, blue syntax and
# notices, green passed outcomes, amber unfinished work, red failures and gray
# metadata. Labels are bold. Lines that start with `$ ` are shown as prompts.
set -euo pipefail
exec cargo run --quiet --locked \
    --manifest-path "$(dirname -- "${BASH_SOURCE[0]}")/../tools/terminal-html/Cargo.toml"
