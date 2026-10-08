# Stage-suggestion harness

This development executable collects stage suggestions for codec and metric
operations. Grouping remains in `how-far-really`, enabled through its
`stage-suggestions` feature.

The manifest expects this checkout layout (the parent directory can be anywhere):

```text
work/
  how-far/
  zen/{zenflate,zenpng,zenjpeg,zenwebp,zengif,zenbitmaps,fast-ssim2,zenzop,zenquant}/
  butteraugli/
```

Paths are relative to this manifest. `zenjpeg`, `fast-ssim2`, and `butteraugli`
refer to member crates within their sibling checkouts. Path patches use the
same layout; git patches retain their declared revisions. Optional codec
features still require their local manifests for Cargo resolution.

From the how-far root, `just stage-metadata` checks the full dependency graph.
The harness is a separate workspace; ordinary workspace tests do not build
these codecs. See the [measurement record](../../benchmarks/stage-suggestions-2026-10-07.md)
for cases, provenance, commands, traces, and interpretation limits.
