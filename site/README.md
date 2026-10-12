# Public guide

The guide is published at https://imazen.github.io/how-far/ by
[the Pages workflow](../.github/workflows/pages.yml). The crate READMEs and
`docs/` are the content source; edit those files instead of generated pages.

Install Zola 0.22.1 and Node 26, then run `just site-check` from the repository
root. This builds the pages and checks desktop/mobile navigation, local links,
and all six interactive traces in Chromium: export, cancellation, fork–join,
skipping, smoothing and diagnostics.
For a first browser setup, run `cd site && npm ci && npx playwright install chromium`.

`dev/build-site.py` writes only under `target/site-src` and
`target/site-preview/how-far`. It converts repository links into guide links,
hides Rustdoc setup lines, and records the event player's data by running
`examples/how-far-app/examples/site_trace.rs`. Snapshot values come from the
tracker; event order and playback speed are illustrative, not measurements. The smoothing
comparison calls the actual Rust `ProgressSmoother`; diagnostic findings come
from `how-far-really`. Autoplay runs while the demo is visible, cycles through
examples, and stops on manual controls. Reduced-motion preferences disable it.

To preview, serve `target/site-preview` with a static HTTP server on port 3035
and open `/how-far/`. The site is static: no analytics, account or external font
service is required. A failed trace load leaves the written guides available.

The player reserves a stable panel height across scenarios and advances work
without pauses for explanatory text. The step controls still expose every
result handoff. Hatched segments mark the tracker's unresolved budget share.
