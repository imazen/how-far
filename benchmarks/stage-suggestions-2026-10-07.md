# Stages zen codecs could declare — 2026-10-07

Where each operation checks its `Stop`, which crate that code is in, and how its
work splits into stages. Every check was timed by how-far-really's profiler with
the `stage-suggestions` feature, and `SuggestedStages` proposed stages from when
each source line was active.

- Harness: `dev/stage-suggestions` (cases from imazen/enough's
  `dev/cancel-latency`, deterministic synthetic inputs), at the commit that adds
  this record, on top of `fd08d23`. Command: `cargo build --release`, then
  `target/release/stage-suggestions --json DIR`. Each span covers only the
  operation: it starts after input preparation and ends before the outputs are
  dropped. Raw traces: `stage-suggestions-2026-10-07/*.json`.
- Codecs, built from local checkouts: zenflate `83b1bdd` (0.4.0, used by the
  zenflate cases), zenpng `27393ee`, zenjpeg `d06d19ae` (one uncommitted file,
  an example, is not built for a dependency), zenwebp `ee9dfc5`, zengif
  `c194dcc`, zenquant `88761c7`, zenbitmaps `edc6ed5`, butteraugli `b143f2b`,
  fast-ssim2 `73f938d`, zenzop `214556d`. zenpng compresses through zenflate
  0.3.6 from crates.io (it requires `^0.3.2`). zencodec `2031094` and zenpixels
  `789d69d` come from git, as zenpng pins them. zenavif was left out: its
  checkout holds uncommitted encoder work.
- Host `r5900xt` (Ryzen 9 5900XT, Zen 3), rustc 1.99.0, release build. Every
  check takes a lock and reads the clock, so wall times include that cost;
  uninstrumented times were not measured.

## By operation

From the full run. Shares are of the operation's wall time. "Unchecked" is time
with no check at all, so nothing in it can be cancelled or reported.

| Operation | Wall | Checks | Crates | Stages found | Best split |
| --- | ---: | ---: | --- | --- | --- |
| zenflate effort 200, 16 MB | 483 s | 201,285 | zenflate | none: one loop throughout | one stage; units = optimal-parse iterations per block (`full_optimal.rs:1096` is 98%) |
| zenflate effort 200, 256 KB | 7.5 s | 3,265 | zenflate | none | as above (93% at `full_optimal.rs:1096`) |
| zenpng Maniac encode, 2048² | 109 s | 1,080,787 | zenflate 91%, zenpng 9% | trial compressions 86%, filter search 3% + 11% | as found: compression trials in zenflate 0.18–87.3 s, then zenpng's filter searches (`filter.rs:361`, `:640`) |
| zenpng decode | 37.4 ms | 2,048 | zenpng | setup 3%, rows 97% | one stage, units = rows (`decoder/mod.rs:315`, 2,047 checks) |
| zenjpeg progressive encode, 4K | 122 ms | 271 | zenjpeg | strips 35%, then 65% unchecked | strips (`encode/streaming.rs:670`), then progressive entropy coding: **79 ms unchecked** |
| zenjpeg progressive decode, 4K | 86.5 ms | 493 | zenjpeg | 4 | scan parse 17% (`parser/progressive.rs:229`), Huffman decode 40% (`entropy/decoder.rs:1717`, `:2131`), reconstruction 43%: **36 ms unchecked** |
| zenwebp lossy m6, 1024² RGBA | 474 ms | 917 | zenwebp | 4 | VP8 color 18% (`vp8/mod.rs:2020`), alpha plane as VP8L 82% (`vp8l/transforms.rs:545` 72%, `:2033` 3%, `cost_model.rs:590` 7%) |
| zenwebp lossless, 2048² | 1.31 s | 3,104 | zenwebp | 9 | transforms 68% (`transforms.rs:474`/`:545` 40%, `:751` 2%, `:1470` 26%), backward refs 13%, entropy coding 19% (`cost_model.rs` 7%, `meta_huffman.rs:856` 7%, `encode.rs:1350` 5%) |
| zengif encode, 64 × 512² frames | 4.78 s | 22,431 | zenquant 99%, zengif 1% | 3 | global histogram 31% (zenquant `histogram.rs:184`), median cut 3%, palette and remap 66% (zenquant `lib.rs:1415`); units = frames (zengif `encoder.rs:543` checks once per frame) |
| zenbitmaps PAM, 8K | 62.4 ms | 272 | zenbitmaps | none | one stage of rows (`pnm/encode.rs:63`) |
| zenzop squeeze, 4 MB | 14.1 s | 601,552 | zenzop | none: the first pass is 1% | one stage; units = squeeze iterations (15, `squeeze.rs:681`), after a 142 ms greedy LZ77 pass (`lz77.rs:217`) |
| butteraugli, 2048² | 736 ms | 2,079 | butteraugli | 2 | reference precompute 45%, **unchecked for its first 333 ms** (`precompute.rs:1084` is the first check), then blur and comparison 55% (`blur.rs:453`, `malta.rs:1487`) |
| fast-ssim2, 2048² | 660 ms | 793 | fast-ssim2 | 2 | conversion 8% (`pipeline/mod.rs:270`), then 6 scales 92%: weight scales by pixel count (1, 1/4, 1/16, …), since the same lines run at every scale |

## Repeatability

A second run of the 11 shorter cases found the same stages in 8, with weights
within 4 points. In zenwebp lossy and zengif, a stage near the 2% fold
threshold appeared in one run and not the other (zenwebp's `backward_refs.rs`,
zengif's `lib.rs:1769`). butteraugli, which runs its checks on several threads,
split differently: 45% then 55% in one run, 62%, 10% and 28% in the other.
Earlier versions of the grouping were less stable; the one used here takes
lines in order of how long they were active, which varies less between runs
than their shares of time.

## What the call sites can and can't split

`SuggestedStages` found stages wherever an operation runs different code in
turn: zenjpeg, zenwebp, zenpng's trials and filter searches, zengif's histogram
and palette phases, and setup before a first check. It found none, correctly,
where one loop runs throughout (zenflate, zenzop, zenbitmaps). It can't split
repeated work whose stages share source lines: zengif's 64 frames and
fast-ssim2's 6 scales. There the unit is the repetition, and a weight comes
from a parameter such as pixels per scale, which a multi-run calibration
against registered parameters would fit.

The stage after the last check found three stretches with no check at all:
zenjpeg's progressive encode (79 ms of 122) and decode (36 ms of 86.5) at 4K,
and butteraugli's first 333 ms. Each is also a cancellation gap.

zenwebp's lossy method 6 spends 82% of its time encoding the alpha plane
losslessly. The input's alpha is a smooth wave (192 ± 60), so this is the
method's alpha effort, not noise in the input.

zenpng's Maniac encode spends 91% of its time inside zenflate, checking 993,396
times in 109 s.

Limits: synthetic inputs, one configuration per codec, and weights from wall
time. Check them across inputs before relying on them.
