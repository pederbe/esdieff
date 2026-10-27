#!/usr/bin/env bash
# Build the website into target/site: the landing page at / and the mdBook docs at /docs/.
# Never deploys. Run `--serve` to preview the result with wrangler, which applies the
# same routing and headers as Cloudflare Workers static assets.
set -euo pipefail
cd "$(dirname "$0")/.."

MDBOOK="${MDBOOK:-mdbook}"
STAGE=target/site-stage
OUT=target/site
NEW=target/site-new
SITE=https://esdieff.com
GITHUB=https://github.com/pederbe/esdieff/blob/main

rm -rf "$STAGE" "$NEW"
mkdir -p "$STAGE" "$NEW"
trap 'rm -rf "$STAGE" "$NEW"' EXIT

# Stage the documentation so links that leave docs/ can be rewritten for the site.
cp book.toml "$STAGE/"
cp -r docs "$STAGE/docs"
rm -rf "$STAGE/docs/agents" "$STAGE/docs/communications" "$STAGE/docs/presentations" \
       "$STAGE/docs/profiling" "$STAGE/docs/profiling.md"
cp CONTEXT.md "$STAGE/docs/domain-language.md"
find "$STAGE/docs" -name '*.md' -print0 | xargs -0 sed -i \
  -e 's#](\.\./\.\./CONTEXT\.md#](../domain-language.md#g' \
  -e 's#](\.\./CONTEXT\.md#](domain-language.md#g' \
  -e "s#](\.\./\.\./examples/#]($GITHUB/examples/#g" \
  -e "s#](\.\./examples/#]($GITHUB/examples/#g" \
  -e "s#](\.\./\.\./src/#]($GITHUB/src/#g" \
  -e "s#](\.\./src/#]($GITHUB/src/#g"
# The domain language file links to docs/ from the repository root.
sed -i -e 's#](docs/#](#g' "$STAGE/docs/domain-language.md"

"$MDBOOK" build "$STAGE" --dest-dir "$PWD/$NEW/docs"
cp -r site/. "$NEW/"

# Search engines get one sitemap for the landing page and every docs chapter.
{
  printf '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n  <url><loc>%s/</loc></url>\n' "$SITE"
  find "$NEW/docs" -name '*.html' -not -name '404.html' -not -name 'print.html' -not -name 'toc.html' -not -name 'README.html' \
    | sed -e "s#^$NEW##" -e 's#/index\.html$#/#' -e 's#\.html$##' | sort \
    | while IFS= read -r path; do printf '  <url><loc>%s%s</loc></url>\n' "$SITE" "$path"; done
  printf '</urlset>\n'
} > "$NEW/sitemap.xml"

# Every local link in the built HTML must resolve to a file in the output.
# The check runs on the new build, so a failure leaves the previous target/site intact.
missing=0
while IFS=: read -r file href; do
  href=${href%%#*}
  href=${href%%\?*}
  [ -z "$href" ] && continue
  if [ "${href#/}" != "$href" ]; then target="$NEW$href"; else target="$(dirname "$file")/$href"; fi
  if [ -d "$target" ]; then target="${target%/}/index.html"; fi
  if [ ! -e "$target" ] && [ ! -e "$target.html" ]; then echo "broken link: $file -> $href"; missing=1; fi
done < <(grep -roE 'href="[^"]+"' --include='*.html' "$NEW" \
  | sed -E 's/:href="([^"]*)"$/:\1/' \
  | grep -vE ':(https?:|//|mailto:|#|javascript:|data:)' \
  | sort -u)
[ "$missing" = 0 ] || { echo "link check failed"; exit 1; }

rm -rf "$OUT"
mv "$NEW" "$OUT"
echo "built $OUT"

if [ "${1:-}" = --serve ]; then
  exec npx --prefix tools/site wrangler dev --config wrangler.jsonc --port 8787
fi
