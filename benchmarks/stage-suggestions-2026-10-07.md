# Stages zen codecs could declare — 2026-10-07

Where each operation checks its `Stop`, which crate those checks are in, and
which stages `SuggestedStages` proposes from when each source line was active.
Every check was timed by how-far-really's profiler with the
`stage-suggestions` feature.

- Harness: `dev/stage-suggestions` (cases from imazen/enough's
  `dev/cancel-latency`, deterministic synthetic inputs), at the commit that
  adds this version of the record. Each span covers only the operation: it
  starts after input preparation and ends before the outputs are dropped. Each
  trace's metadata names its case, codec and operation.
- Runs: `stage-suggestions --json run-1` (all 13 cases) and
  `stage-suggestions --json run-2 <11 cases>` (all but the two longest), one
  after the other. Traces in `stage-suggestions-2026-10-07/run-1` and `run-2`,
  console output in `run-1.log` and `run-2.log`. The tables below come from
  `dev/stage-suggestions/summarize.py traces run-1` and
  `summarize.py compare run-1.log run-2.log`.
- Codecs, built from local checkouts: zenflate `83b1bdd` (0.4.0, used by the
  zenflate cases), zenpng `27393ee`, zenjpeg `d06d19ae` (one uncommitted file,
  an example, is not built for a dependency), zenwebp `ee9dfc5`, zengif
  `c194dcc`, zenquant `88761c7`, zenbitmaps `edc6ed5`, butteraugli `b143f2b`,
  fast-ssim2 `73f938d`, zenzop `214556d`. zenpng compresses through zenflate
  0.3.6 from crates.io (it requires `^0.3.2`). zencodec `2031094` and zenpixels
  `789d69d` come from git, as zenpng pins them. The harness's lockfile is not
  committed; zenanalyze comes from git without a pinned revision. zenavif was
  left out: its checkout holds uncommitted encoder work.
- Host `r5900xt` (Ryzen 9 5900XT, Zen 3), rustc 1.99.0, release build. Other
  jobs shared the box (peak load 22 of 32 threads in run 1), so wall times are
  not comparable across runs. Every check takes a lock and reads the clock;
  uninstrumented times were not measured.

## What the numbers mean

Three different quantities appear below, and they are not interchangeable:

1. **Wall**: the span, from the operation's start to its return.
2. **Checked intervals**: each source line's time is the intervals that *end*
   at its calls, from the previous timed call or the span's start. A crate's
   share is of intervals ending at that crate's checks. It is not time spent
   inside the crate: an interval can include caller work and other crates.
   The unchecked head (span start to the first call) is part of the first
   line's interval; the unchecked tail (after the last call) ends at no line.
3. **Candidate weights**: `SuggestedStages`' whole-percent weights. Time at
   outer-loop lines that span several stretches is spread over the stretches
   in proportion; the finding states how much that was.

## Wall, unchecked time and crates (run 1)

| Case | Wall ms | Checks | Unchecked head ms | Unchecked tail ms | Checked intervals by crate (% of wall) |
| --- | ---: | ---: | ---: | ---: | --- |
| butteraugli-2048 | 1071.6 | 2079 | 458.2 | 14.6 | butteraugli 98.6 |
| fast-ssim2-2048 | 951.1 | 793 | 0.0 | 0.0 | fast-ssim2 100.0 |
| zenbitmaps-pam-8k | 79.0 | 272 | 0.0 | 0.0 | zenbitmaps 100.0 |
| zenflate-effort200-16mb | 680123.7 | 201285 | 0.6 | 36.7 | zenflate 100.0 |
| zenflate-effort200-small | 9039.1 | 3265 | 0.2 | 1.3 | zenflate 100.0 |
| zengif-encode-64f | 7883.7 | 22431 | 0.0 | 0.0 | zenquant 98.8, zengif 1.2 |
| zenjpeg-decode-progressive-4k | 109.1 | 493 | 0.0 | 41.3 | zenjpeg 62.2 |
| zenjpeg-encode-progressive-4k | 142.0 | 271 | 0.4 | 93.6 | zenjpeg 34.1 |
| zenpng-decode-balanced | 41.9 | 2048 | 1.4 | 0.0 | zenpng 100.0 |
| zenpng-maniac-2048 | 119428.1 | 1080787 | 45.6 | 53.5 | zenflate 90.6, zenpng 9.4 |
| zenwebp-lossless-2048 | 2148.0 | 3104 | 38.6 | 3.1 | zenwebp 99.9 |
| zenwebp-lossy-m6-1024 | 541.3 | 917 | 0.0 | 0.1 | zenwebp 100.0 |
| zenzop-squeeze-enhanced-4mb | 21895.8 | 601552 | 0.8 | 37.0 | zenzop 99.8 |

Three long stretches have no check at all, so nothing in them can be cancelled
or reported. Two are tails, inside calls that take a `Stop`: zenjpeg's
progressive encode (93.6 of 142 ms, in `encode_bytes`) and decode (41.3 of
109 ms, in `decode`). One is a head: butteraugli's first 458 ms, in
`ButteraugliReference::new`, which takes no `Stop` (only butteraugli's
`compare_*_with_stop` methods do).

## Candidate stages and the best split (run 1)

Weights are candidate weights (quantity 3). "Outer" is the share of checked
intervals at outer-loop lines that the weights spread over the stretches. The
best-split column is a reading of these weights and of the source lines, not
a measurement of each part's cost.

| Case | Candidate weights | Outer | Best split |
| --- | --- | ---: | --- |
| zenflate effort 200, 16 MB and 256 KB | none: one loop runs throughout | — | one stage; units = optimal-parse iterations per block (`full_optimal.rs:1096` has nearly all checks) |
| zenpng Maniac encode, 2048² | zenflate `compress/mod.rs:2099` 86, zenpng `filter.rs:361` 3, `filter.rs:640` 11 | 34% | compression trials, then filter searches; a third of the time was spread to reach these weights |
| zenpng decode (Balanced input) | `decoder/mod.rs:298` 3, `:315` 97 | 0 | one stage, units = rows (`decoder/mod.rs:315`, 2,047 checks) |
| zenjpeg progressive encode, 4K | strips 34 (`encode/streaming.rs:670`), after it 66 | 0 | strips, then progressive entropy coding, which needs checks |
| zenjpeg progressive decode, 4K | `parser/progressive.rs:229` 20, `entropy/decoder.rs:1717` 10, `:2131` 31, after `parser/mod.rs:820` 39 | 2% | scan parsing, Huffman decoding, then reconstruction, which needs checks |
| zenwebp lossy m6, 1024² RGBA | `vp8/mod.rs:2020` 22, `vp8l/transforms.rs:2033` 3, `:545` 68, `cost_model.rs:590` 7 | 7% | VP8 color, then the lossless alpha path (`vp8l`), which carries 78 of the weight; the input's alpha is a smooth wave (192 ± 60), and one input doesn't show why |
| zenwebp lossless, 2048² | `transforms.rs:474` 2, `:545` 37, `:1470` 24, `backward_refs.rs:520` 13, `cost_model.rs:590` 4, `:686` 4, `meta_huffman.rs:856` 10, `encode.rs:1350` 6 | 27% | transforms, backward references, entropy coding |
| zengif encode, 64 × 512² frames | zenquant `histogram.rs:184` 29, `median_cut.rs:29` 3, `lib.rs:1415` 68 | 44% | global histogram, then palette and remap; units = frames (zengif `encoder.rs:543` checks once per frame) |
| zenbitmaps PAM, 8K, encode then decode | none | — | the case times encode and decode together; split the span before drawing conclusions about either |
| zenzop squeeze, 4 MB | none: the greedy first pass is too small to stand alone | — | one stage; units = squeeze iterations (15 at `squeeze.rs:681`) |
| butteraugli, 2048² | `precompute.rs:1084` 43, `blur.rs:453` 57 | 0 | reference precompute (in `new`, unchecked), then comparison |
| fast-ssim2, 2048² | `pipeline/mod.rs:270` 9, `pipeline/simd.rs:736` 91 | 0 | conversion, then 6 scales; the same lines run at every scale, so weight scales by pixel count |

`SuggestedStages` finds stages wherever an operation runs different code in
turn, and none where one loop runs throughout. It can't split repeated work
that runs the same lines: zengif's 64 frames, fast-ssim2's 6 scales. There the
unit is the repetition, and a weight would come from a parameter such as
pixels per scale. Where outer-loop time is a large share (zengif 44%, zenpng
Maniac 34%, zenwebp lossless 27%), the weights depend on how that time is
spread and are weak evidence.

## Repeatability

| Case | Run 1 | Run 2 | Same stages |
| --- | --- | --- | --- |
| zenflate-effort200-small | none | none | yes |
| zenpng-decode-balanced | mod.rs:298 3, mod.rs:315 97 | mod.rs:298 8, mod.rs:315 92 | yes |
| zenjpeg-encode-progressive-4k | streaming.rs:670 34, after streaming.rs:670 66 | streaming.rs:670 36, after streaming.rs:670 64 | yes |
| zenjpeg-decode-progressive-4k | progressive.rs:229 20, decoder.rs:1717 10, decoder.rs:2131 31, after mod.rs:820 39 | progressive.rs:229 20, decoder.rs:1717 10, decoder.rs:2131 30, after mod.rs:820 40 | yes |
| zenwebp-lossy-m6-1024 | mod.rs:2020 22, transforms.rs:2033 3, transforms.rs:545 68, cost_model.rs:590 7 | mod.rs:2020 18, transforms.rs:2033 3, transforms.rs:545 72, cost_model.rs:590 7 | yes |
| zenwebp-lossless-2048 | 8 stretches | the same 8 plus `transforms.rs:751` 2 | no |
| zengif-encode-64f | histogram.rs:184 29, median_cut.rs:29 3, lib.rs:1415 68 | lib.rs:1769 2, histogram.rs:184 30, median_cut.rs:29 3, lib.rs:1415 65 | no |
| zenbitmaps-pam-8k | none | none | yes |
| zenzop-squeeze-enhanced-4mb | none | none | yes |
| butteraugli-2048 | precompute.rs:1084 43, blur.rs:453 57 | precompute.rs:1084 60, blur.rs:1807 10, malta.rs:1487 27, after precompute.rs:1667 3 | no |
| fast-ssim2-2048 | mod.rs:270 9, simd.rs:736 91 | mod.rs:270 7, simd.rs:736 93 | yes |

8 of 11 cases found the same stages, with weights within 5 points. In zenwebp
lossless and zengif, a stage near the 2% fold threshold appeared in one run
only. butteraugli, which checks from several threads, split differently. Two
runs under shared load are a small sample.

Limits: synthetic inputs, one configuration per codec, weights from wall time
under shared load. Check them across inputs before relying on them.
