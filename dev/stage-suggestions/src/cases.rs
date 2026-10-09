//! One case per codec operation, from imazen/enough's dev/cancel-latency
//! harness (adapted: each case takes any `&dyn Stop`). The stop never
//! fires, so every run completes and reports all of its checks.

use enough::{Stop, Unstoppable};
use std::time::Instant;

pub struct Case {
    pub name: &'static str,
    pub codec: &'static str,
    pub blurb: &'static str,
    /// Runs the case; it calls `begin` just before the measured operation,
    /// after preparing its input, and `end` when the operation returns,
    /// before its buffers are dropped.
    pub run: fn(&dyn Stop, &dyn Fn(), &dyn Fn()) -> Result<String, String>,
}

pub fn err<E: core::fmt::Display>(e: E) -> String {
    format!("{e}")
}

/// Build the scenario registry for the enabled features.
pub fn all() -> Vec<Case> {
    let mut v: Vec<Case> = Vec::new();

    #[cfg(feature = "zenflate")]
    {
        v.push(Case {
            name: "zenflate-effort200-16mb",
            codec: "zenflate",
            blurb: "deflate_compress at effort 200 (30+ optimal-parse iterations) on 16MB mixed data",
            run: |m, begin, end| {
                let data = crate::inputs::bytes_mixed(16 << 20, 0xF1A7E);
                let mut c =
                    zenflate::compress::Compressor::new(zenflate::compress::CompressionLevel::new(
                        200,
                    ));
                let bound =
                    zenflate::compress::Compressor::deflate_compress_bound(data.len());
                let mut out = vec![0u8; bound];
                begin();
                let t = Instant::now();
                let n = c
                    .deflate_compress(&data, &mut out, m)
                    .map_err(err)?;
                end();
                Ok(format!(
                    "{}B -> {}B in {:?}",
                    data.len(),
                    n,
                    t.elapsed()
                ))
            },
        });
        v.push(Case {
            name: "zenflate-effort200-small",
            codec: "zenflate",
            blurb: "effort 200 on 256KB — checks whether small inputs still see poll traffic",
            run: |m, begin, end| {
                let data = crate::inputs::bytes_mixed(256 << 10, 0xCAFE);
                let mut c = zenflate::compress::Compressor::new(
                    zenflate::compress::CompressionLevel::new(200),
                );
                let bound = zenflate::compress::Compressor::deflate_compress_bound(data.len());
                let mut out = vec![0u8; bound];
                begin();
                let t = Instant::now();
                let n = c.deflate_compress(&data, &mut out, m).map_err(err)?;
                end();
                Ok(format!("{}B -> {}B in {:?}", data.len(), n, t.elapsed()))
            },
        });
    }

    #[cfg(feature = "zenpng")]
    {
        v.push(Case {
            name: "zenpng-maniac-2048",
            codec: "zenpng",
            blurb: "encode_rgb8 Compression::Maniac on 2048x2048 (both cancel+deadline metered)",
            run: |m, begin, end| {
                use rgb::FromSlice;
                let px = crate::inputs::rgb8_photo(2048, 2048, 7);
                let img = imgref::ImgRef::new(px.as_rgb(), 2048, 2048);
                let cfg =
                    zenpng::EncodeConfig::default().with_compression(zenpng::Compression::Maniac);
                begin();
                let t = Instant::now();
                let out = zenpng::encode_rgb8(img, None, &cfg, m, m).map_err(err)?;
                end();
                Ok(format!("2048x2048 -> {}B in {:?}", out.len(), t.elapsed()))
            },
        });
        v.push(Case {
            name: "zenpng-decode-balanced",
            codec: "zenpng",
            blurb: "decode a Balanced-compressed 2048x2048 PNG (decode poll pattern)",
            run: |m, begin, end| {
                use rgb::FromSlice;
                // Encode unmetered first — this case isolates DECODE polling.
                let px = crate::inputs::rgb8_photo(2048, 2048, 7);
                let img = imgref::ImgRef::new(px.as_rgb(), 2048, 2048);
                let cfg =
                    zenpng::EncodeConfig::default().with_compression(zenpng::Compression::Balanced);
                let png = zenpng::encode_rgb8(img, None, &cfg, &Unstoppable, &Unstoppable)
                    .map_err(err)?;
                begin();
                let t = Instant::now();
                let out =
                    zenpng::decode(&png, &zenpng::PngDecodeConfig::default(), m).map_err(err)?;
                end();
                Ok(format!(
                    "{}B png -> {}x{} in {:?}",
                    png.len(),
                    out.info.width,
                    out.info.height,
                    t.elapsed()
                ))
            },
        });
    }

    #[cfg(feature = "zenjpeg")]
    {
        v.push(Case {
            name: "zenjpeg-encode-progressive-4k",
            codec: "zenjpeg",
            blurb: "EncodeRequest progressive encode of 3840x2160 RGBA",
            run: |m, begin, end| {
                let px = crate::inputs::rgba8_photo(3840, 2160, 11);
                let cfg = zenjpeg::encoder::EncoderConfig::rgb(90)
                    .progressive(zenjpeg::encoder::ProgressiveScanMode::Smallest);
                begin();
                let t = Instant::now();
                let jpeg = cfg
                    .request()
                    .stop(m)
                    .encode_bytes(&px, 3840, 2160, zenjpeg::encoder::PixelLayout::Rgba8Srgb)
                    .map_err(err)?;
                end();
                Ok(format!(
                    "3840x2160 -> {}B jpeg in {:?}",
                    jpeg.len(),
                    t.elapsed()
                ))
            },
        });
        v.push(Case {
            name: "zenjpeg-decode-progressive-4k",
            codec: "zenjpeg",
            blurb: "DecodeConfig::decode on a large progressive JPEG",
            run: |m, begin, end| {
                let px = crate::inputs::rgba8_photo(3840, 2160, 11);
                let cfg = zenjpeg::encoder::EncoderConfig::rgb(90)
                    .progressive(zenjpeg::encoder::ProgressiveScanMode::Smallest);
                let jpeg = cfg
                    .request()
                    .encode_bytes(&px, 3840, 2160, zenjpeg::encoder::PixelLayout::Rgba8Srgb)
                    .map_err(err)?;
                begin();
                let t = Instant::now();
                let out = zenjpeg::decoder::DecodeConfig::new()
                    .decode(&jpeg, m)
                    .map_err(err)?;
                end();
                Ok(format!(
                    "{}B jpeg -> {}x{} in {:?}",
                    jpeg.len(),
                    out.width(),
                    out.height(),
                    t.elapsed()
                ))
            },
        });
    }

    #[cfg(feature = "zenwebp")]
    {
        v.push(Case {
            name: "zenwebp-lossy-m6-1024",
            codec: "zenwebp",
            blurb: "EncodeRequest::lossy method=6 (full trellis) on 1024x1024 RGBA",
            run: |m, begin, end| {
                let px = crate::inputs::rgba8_photo(1024, 1024, 13);
                let mut cfg = zenwebp::LossyConfig::new();
                cfg.quality = 75.0;
                cfg.method = 6;
                begin();
                let t = Instant::now();
                let webp = zenwebp::EncodeRequest::lossy(
                    &cfg,
                    &px,
                    zenwebp::PixelLayout::Rgba8,
                    1024,
                    1024,
                )
                .with_stop(m)
                .encode()
                .map_err(err)?;
                end();
                Ok(format!(
                    "1024x1024 -> {}B webp in {:?}",
                    webp.len(),
                    t.elapsed()
                ))
            },
        });
        v.push(Case {
            name: "zenwebp-lossless-2048",
            codec: "zenwebp",
            blurb: "lossless encode 2048x2048 (VP8L backward-refs heavy path)",
            run: |m, begin, end| {
                let px = crate::inputs::rgba8_photo(2048, 2048, 17);
                let cfg = zenwebp::LosslessConfig::default();
                begin();
                let t = Instant::now();
                let webp = zenwebp::EncodeRequest::lossless(
                    &cfg,
                    &px,
                    zenwebp::PixelLayout::Rgba8,
                    2048,
                    2048,
                )
                .with_stop(m)
                .encode()
                .map_err(err)?;
                end();
                Ok(format!(
                    "2048x2048 -> {}B lossless webp in {:?}",
                    webp.len(),
                    t.elapsed()
                ))
            },
        });
    }

    #[cfg(feature = "zengif")]
    {
        v.push(Case {
            name: "zengif-encode-64f",
            codec: "zengif",
            blurb: "encode_gif of 64 animated 512x512 frames (palette quantization)",
            run: |m, begin, end| {
                use rgb::FromSlice;
                let frames: Vec<zengif::FrameInput> = (0..64)
                    .map(|i| {
                        let px = crate::inputs::rgba8_photo(512, 512, 100 + i);
                        zengif::FrameInput::new(
                            512,
                            512,
                            4,
                            px.as_rgba()
                                .to_vec()
                                .into_iter()
                                .map(|p| zengif::Rgba::new(p.r, p.g, p.b, p.a))
                                .collect(),
                        )
                    })
                    .collect();
                begin();
                let t = Instant::now();
                let gif = zengif::encode_gif(
                    frames,
                    512,
                    512,
                    zengif::EncoderConfig::new(),
                    zengif::Limits::default(),
                    m,
                )
                .map_err(err)?;
                end();
                Ok(format!(
                    "64 frames -> {}B gif in {:?}",
                    gif.len(),
                    t.elapsed()
                ))
            },
        });
    }

    #[cfg(feature = "zenbitmaps")]
    {
        v.push(Case {
            name: "zenbitmaps-pam-8k",
            codec: "zenbitmaps",
            blurb: "encode_pam + decode roundtrip of 8K RGBA — fast path, poll-storm watch",
            run: |m, begin, end| {
                let px = crate::inputs::rgba8_photo(7680, 4320, 23);
                begin();
                let t = Instant::now();
                let pam =
                    zenbitmaps::encode_pam(&px, 7680, 4320, zenbitmaps::PixelLayout::Rgba8, m)
                        .map_err(err)?;
                let dec = zenbitmaps::decode(&pam, m).map_err(err)?;
                end();
                Ok(format!(
                    "{}B pam, decoded {} bytes in {:?}",
                    pam.len(),
                    dec.pixels().len(),
                    t.elapsed()
                ))
            },
        });
    }

    #[cfg(feature = "zenzop")]
    {
        v.push(Case {
            name: "zenzop-squeeze-enhanced-4mb",
            codec: "zenzop",
            blurb: "enhanced zopfli, maxblocks=1 (largest DP per squeeze iter) on 4MB mixed",
            run: |m, begin, end| {
                use std::io::Write;
                let data = crate::inputs::bytes_mixed(4 << 20, 0x20F1);
                let mut opts = zenzop::Options::default();
                opts.enhanced = true;
                // One block per 1MB master chunk: the squeeze loop checks
                // once per iteration, so this maximizes per-check work.
                opts.maximum_block_splits = 1;
                begin();
                let t = Instant::now();
                let mut enc = zenzop::DeflateEncoder::with_stop(opts, Vec::new(), m);
                enc.write_all(&data).map_err(err)?;
                let res = enc.finish().map_err(err)?;
                let out = res.into_inner();
                end();
                Ok(format!(
                    "{}B -> {}B in {:?}",
                    data.len(),
                    out.len(),
                    t.elapsed()
                ))
            },
        });
    }

    #[cfg(feature = "butteraugli")]
    {
        v.push(Case {
            name: "butteraugli-2048",
            codec: "butteraugli",
            blurb: "compare_with_stop on two 2048x2048 sRGB images",
            run: |m, begin, end| {
                let a = crate::inputs::rgb8_photo(2048, 2048, 31);
                let b = crate::inputs::rgb8_photo(2048, 2048, 37);
                begin();
                let t = Instant::now();
                let reference = butteraugli::precompute::ButteraugliReference::new(
                    &a,
                    2048,
                    2048,
                    butteraugli::ButteraugliParams::default(),
                )
                .map_err(err)?;
                let res = reference.compare_with_stop(&b, m).map_err(err)?;
                end();
                Ok(format!("score {:.3} in {:?}", res.score, t.elapsed()))
            },
        });
    }

    #[cfg(feature = "fast-ssim2")]
    {
        v.push(Case {
            name: "fast-ssim2-2048",
            codec: "fast-ssim2",
            blurb: "compute_ssimulacra2_with_config on 2048x2048 RGB8 pair (simd)",
            run: |m, begin, end| {
                let a = crate::inputs::rgb8_photo(2048, 2048, 41);
                let b = crate::inputs::rgb8_photo(2048, 2048, 43);
                let src = fast_ssim2::PixelSlice::new(
                    &a,
                    2048,
                    2048,
                    2048 * 3,
                    fast_ssim2::PixelDescriptor::RGB8_SRGB,
                )
                .map_err(err)?;
                let dst = fast_ssim2::PixelSlice::new(
                    &b,
                    2048,
                    2048,
                    2048 * 3,
                    fast_ssim2::PixelDescriptor::RGB8_SRGB,
                )
                .map_err(err)?;
                let cfg = fast_ssim2::Ssimulacra2Config::simd().with_stop(m);
                begin();
                let t = Instant::now();
                let score =
                    fast_ssim2::compute_ssimulacra2_with_config(&src, &dst, &cfg).map_err(err)?;
                end();
                Ok(format!("ssim2 {:.3} in {:?}", score, t.elapsed()))
            },
        });
    }

    v
}
