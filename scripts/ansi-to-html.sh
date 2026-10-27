#!/usr/bin/env bash
# Turn Esdieff's coloured terminal output into an HTML block for the documentation.
#
#   { echo '$ esdieff check thermostat.sdf.json'; esdieff --color=always check thermostat.sdf.json; } \
#     | scripts/ansi-to-html.sh
#
# The palette is the one the command-line tool uses: bold headings, cyan syntax and
# notices, green passed outcomes, yellow unfinished work and red failures. Lines that
# start with `$ ` are shown as prompts. Any other escape sequence is dropped.
set -euo pipefail
perl -CSD -pe '
  BEGIN { print "<pre class=\"terminal\"><code>"; }
  s/&/&amp;/g; s/</&lt;/g; s/>/&gt;/g;
  s/\e\[1m/<b>/g;
  s/\e\[31m/<span class="r">/g;
  s/\e\[32m/<span class="g">/g;
  s/\e\[33m/<span class="y">/g;
  s/\e\[36m/<span class="c">/g;
  s/\e\[0m/<\/x>/g;
  s/\e\[[0-9;]*m//g;
  # Close whatever the last opening tag was: bold or a span.
  my @open;
  s{(<b>|<span class="[rgyc]">|</x>)}{
    my $t = $1;
    if ($t eq "</x>") { my $o = pop @open; defined $o ? ($o eq "b" ? "</b>" : "</span>") : "" }
    else { push @open, ($t eq "<b>" ? "b" : "s"); $t }
  }ge;
  s/^\$ /<span class="p">\$<\/span> /;
  END { print "</code></pre>\n"; }
'
