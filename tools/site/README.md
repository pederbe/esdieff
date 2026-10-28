# Website build

The Esdieff website is a static site: the landing page in `site/` and the
mdBook documentation in `docs/`, combined by `scripts/build-site.sh` into
`target/site` with the landing page at `/` and the docs at `/docs/`.

## Landing page styles

The landing page uses Tailwind CSS 4 and daisyUI 5. The CSS is compiled at
edit time and committed, so site builds need neither Node.js nor a browser-side
compiler. After changing landing markup, utilities or the themes in
`tools/site/landing.css`, run:

```sh
npm ci --ignore-scripts --no-audit --no-fund --prefix tools/site
npm run build:landing --prefix tools/site
```

Commit the generated `site/landing.css` with its inputs. `landing.css` defines one dark theme from the Fiery Ocean palette. Each swatch
colour keeps one role: primary (brick red) for actions and the glowing second
heading line, secondary (steel blue) for eyebrows, links and code keys, accent
(molten lava) for tints, neutral (deep space blue) for code and terminal panels,
and papaya whip for text. `docs/theme/esdieff.css` and `site/favicon.svg` use
the same colours.

Motion: the hero words rise once on load, each content block fades up as it
enters view, the hero glow breathes, an orbit dot circles the ring, the aura
around the illustration pulses, and the thermostat illustration plays a
14-second loop through the Web Animations API. Loops pause outside the viewport, in hidden tabs, on
hover and focus, and through the page's motion control. Reduced-motion
preferences show the complete static result, and so does a page without
JavaScript.

The main script is deferred. A small inline bootstrap prepares entrances before
the first paint and restores the static page if the main script fails to load.
It is authorized by its exact SHA-256 hash in the landing page's meta
policy and the `/` and `/index.html` policies in `site/_headers`. If you change
the bootstrap, recalculate its hash and update all three policies:

```sh
sed -n 's#.*<script id="entrance-bootstrap">\(.*\)</script>.*#\1#p' site/index.html \
  | tr -d '\n' | openssl dgst -sha256 -binary | openssl base64 -A
```

Use the output after `sha256-`. Keep the stylesheet external and minified.

## Link previews and search metadata

The homepage and docs share the 1200 × 630 social image at `site/og.png`.
Its editable source is `tools/site/social-card.svg`, using the landing page's
Fiery Ocean palette and the thermostat example. With Noto Sans and librsvg
installed, regenerate it from the repository root:

```sh
rsvg-convert tools/site/social-card.svg -o site/og.png
```

Commit both files after editing the card. Normal site builds copy the PNG
without needing fonts or an image renderer. Image dimensions, descriptions
and Twitter card metadata live in `site/index.html` and `docs/theme/head.hbs`.
`book.toml` selects that theme directory so mdBook includes the custom head.
The build stages the homepage favicon there for mdBook to fingerprint.
The build adds a canonical URL and matching Open Graph URL to each docs
chapter using the same clean routes as the sitemap. Generated print, navigation,
redirect and error pages stay out of the sitemap and receive `noindex`.

## Combined build and preview

```sh
scripts/build-site.sh          # builds target/site and checks internal links
scripts/build-site.sh --serve  # also serves it locally with wrangler on port 8787
```

Restart `--serve` after every rebuild. `wrangler dev` indexes the asset folder
once at start, so files with new hashed names 404 until it starts again.

Lighthouse reports "robots.txt is not valid" for the landing page because it
fetches the file from the page itself and the landing CSP forbids fetches with
`connect-src 'none'`. That is a Lighthouse limitation, not a site defect:
`/robots.txt` is served and the CSP stays strict.

Terminal examples in the docs keep the tool's own colours. Capture the output
with `--color=always`, prefix the command line with `$ `, and pipe the text through
`scripts/ansi-to-html.sh`, which writes a `<pre class="terminal">` block to paste into
the Markdown. `docs/theme/esdieff.css` maps the five roles (bold headings, cyan
syntax and notices, green passed, yellow unfinished, red failed) to readable
colours in the light and dark docs themes. Plain ```` ```console ```` fences remain
fine for output without colour.

The build needs mdBook 0.5 (`MDBOOK` can select an installed binary). It stages
`docs/` so that links to the repository root (the domain language and the
examples) resolve on the site, and it never deploys. `wrangler.jsonc` describes
the Cloudflare Workers static-assets deployment.

## Deployment

Website and documentation changes merged into `main` deploy automatically to
esdieff.com after the required CI checks pass. CI calls the existing Website
workflow to build and deploy that revision. Pull requests only validate the
changes. Source-only changes and repository overview edits do not publish the site.
Merged revisions queue without replacing pending runs, so a later source-only
merge cannot cancel an earlier website deployment.

The `WEBSITE_ENABLED` repository variable must be `true`. Deployment uses the
existing `website` environment and its Cloudflare credentials.

You can still start a deployment manually:

```sh
gh workflow run website.yml --ref main -F deploy=true
```

The workflow uses the latest published release. To require a particular release,
add `-f tag=vX.Y.Z`. An explicit tag must match the latest release. Omitting
`deploy=true` builds the website without publishing it.
