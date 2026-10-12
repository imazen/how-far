//! A second library that calls the codec inside its own stages, through one
//! `&dyn Pulse`, without depending on a tracker or diagnostics.
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

use alloc::vec::Vec;
use how_far::{Execution, PhaseSpec, Phases, Stages, StopReason, Total, prelude::*};
use how_far_example_codec::{self as codec, Mode};

/// The pipeline's own error, which keeps the codec's error intact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// An image failed or stopped inside the codec.
    Codec {
        /// The image's position in the batch.
        image: usize,
        /// The codec's error.
        error: codec::Error,
    },
    /// A stop request between images.
    Stopped(StopReason),
    /// Input the pipeline rejects itself.
    Invalid(&'static str),
}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec { image, error } => write!(f, "image {image}: {error}"),
            Self::Stopped(reason) => reason.fmt(f),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Codec { error, .. } => Some(error),
            _ => None,
        }
    }
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl AsStopReason for Error {
    fn as_stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(reason) => Some(*reason),
            // Classification delegates to the nested library's own error.
            Self::Codec { error, .. } => error.as_stop_reason(),
            Self::Invalid(_) => None,
        }
    }
}

fn codec_error(image: usize) -> impl Fn(codec::Error) -> Error {
    move |error| Error::Codec { image, error }
}

/// Decode, transform, and optionally sharpen, using different data and result
/// types at each stage. Hand over each result before `?` so errors resolve the
/// pending stages even when the function returns early.
pub fn convert(
    input: &[u8],
    pulse: &dyn Pulse,
    mode: Mode,
    sharpen: bool,
) -> Result<Vec<u8>, Error> {
    let (mut stages, [decode_key, transform_key, sharpen_key]) = Stages::define(
        pulse,
        [
            PhaseSpec::new("decode", 4, Total::Unknown),
            PhaseSpec::new("transform", 4, Total::Exact(input.len() as u64)),
            PhaseSpec::new("sharpen", 1, Total::Exact(1)),
        ],
    );
    let decode_phase = stages.begin(decode_key);
    let mut pixels = codec::decode(input, &decode_phase, mode)
        .map_err(codec_error(0))
        .finish_phase(decode_phase)?;
    let transform_phase = stages.begin(transform_key);
    invert(&mut pixels, &transform_phase).finish_phase(transform_phase)?;
    if sharpen {
        let sharpen_phase = stages.begin(sharpen_key);
        sharpen_phase.step(1).finish_phase(sharpen_phase)?;
    }
    stages.complete(Ok(pixels))
}

fn invert(pixels: &mut [u8], pulse: &dyn Pulse) -> Result<(), Error> {
    pulse.check()?;
    let mut paced = pulse.paced(64);
    for pixel in pixels {
        *pixel ^= 0xff;
        paced.step(1)?;
    }
    Ok(paced.finish()?)
}

/// Keep the caller's stage and cancellation, adding a library-local stop source.
/// Both are borrowed, so a stack-local token or caller-supplied `&dyn Stop` works.
pub fn convert_with_stop(
    input: &[u8],
    pulse: &dyn Pulse,
    additional_stop: &dyn Stop,
    mode: Mode,
    sharpen: bool,
) -> Result<Vec<u8>, Error> {
    let pulse = how_far::WithStop::borrowed(pulse, additional_stop);
    convert(input, &pulse, mode, sharpen)
}

/// Encode images one after another, each inside its own stage.
pub fn process_each(images: &[codec::Image], pulse: &dyn Pulse) -> Result<Vec<Vec<u8>>, Error> {
    let names: Vec<_> = (0..images.len())
        .map(|i| alloc::format!("image {i}"))
        .collect();
    let parts: Vec<_> = names
        .iter()
        .zip(images)
        .map(|(name, image)| PhaseSpec::new(name, image.pixels.len().max(1) as u64, Total::Unknown))
        .collect();
    let mut stages = Stages::new(pulse, &parts);
    let mut encoded = Vec::with_capacity(images.len());
    for (index, image) in images.iter().enumerate() {
        let stage = stages.begin_next();
        encoded.push(
            codec::encode(image, &stage)
                .map_err(codec_error(index))
                .finish_phase(stage)?,
        );
    }
    stages.complete(Ok(encoded))
}

/// Validate, encode every image in parallel (a fork-join child per image, on
/// Rayon, each handed to the codec, which plans its own stages inside it),
/// then pack.
#[cfg(feature = "std")]
pub fn process(images: &[codec::Image], pulse: &dyn Pulse) -> Result<Vec<Vec<u8>>, Error> {
    use rayon::prelude::*;
    let pixels: u64 = images.iter().map(|image| image.pixels.len() as u64).sum();
    let (mut stages, [validate_key, encode_key, pack_key]) = Stages::define(
        pulse,
        [
            PhaseSpec::new("validate", 1, Total::Exact(images.len() as u64)).units("images"),
            PhaseSpec::new("encode", 20, Total::Unknown),
            PhaseSpec::new("pack", 1, Total::Exact(pixels)).units("bytes"),
        ],
    );
    let validate_phase = stages.begin(validate_key);
    validate(images, &validate_phase).finish_phase(validate_phase)?;
    let encode_phase = stages.begin(encode_key);
    let names: Vec<_> = (0..images.len())
        .map(|i| alloc::format!("image {i}"))
        .collect();
    let parts: Vec<_> = names
        .iter()
        .zip(images)
        .map(|(name, image)| PhaseSpec::new(name, image.pixels.len().max(1) as u64, Total::Unknown))
        .collect();
    let pulse: &dyn Pulse = &encode_phase;
    let result = pulse
        .plan(Execution::ForkJoin, &parts)
        .into_par_iter()
        .zip(images.par_iter())
        .enumerate()
        .map(|(index, (child, image))| {
            codec::encode(image, &child)
                .map_err(codec_error(index))
                .finish_phase(child)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<Result<Vec<_>, _>>();
    let encoded = result.finish_phase(encode_phase)?;
    let pack_phase = stages.begin(pack_key);
    pack(&encoded, &pack_phase).finish_phase(pack_phase)?;
    stages.complete(Ok(encoded))
}

#[cfg(feature = "std")]
fn validate(images: &[codec::Image], pulse: &dyn Pulse) -> Result<(), Error> {
    for _ in images {
        pulse.step(1)?;
    }
    Ok(())
}
#[cfg(feature = "std")]
fn pack(encoded: &[Vec<u8>], pulse: &dyn Pulse) -> Result<(), Error> {
    for bytes in encoded {
        pulse.step(bytes.len() as u64)?;
    }
    Ok(())
}

/// Intentionally incorrect implementations, for the diagnostic demonstration.
#[derive(Clone, Copy, Debug)]
pub enum Bug {
    MissingHandoff,
    EarlyQuestionMark,
    MissingReports,
    SwallowedStop,
    MisclassifiedStop,
    DuplicateStage,
    CountBeforePlan,
    ForgottenRequiredStage,
    ReportToBranch,
}

pub fn buggy(pulse: &dyn Pulse, bug: Bug) -> Result<(), Error> {
    if matches!(bug, Bug::CountBeforePlan) {
        pulse.advance(1);
    }
    let mut phases = Phases::new(
        pulse,
        Execution::Sequence,
        &[
            PhaseSpec::new("work", 1, Total::Exact(2)),
            PhaseSpec::new("required-tail", 1, Total::Exact(1)),
        ],
    );
    if matches!(bug, Bug::ReportToBranch) {
        pulse.advance(1);
    }
    if matches!(bug, Bug::EarlyQuestionMark) {
        // The original error reaches the caller, but phases never sees it.
        phases.run(0, |_| Err::<(), _>(Error::Invalid("bad input")))?;
    }
    let result = phases.run(0, |p| {
        match bug {
            Bug::SwallowedStop => {
                let _ = p.check();
            }
            Bug::MisclassifiedStop => {
                p.check().map_err(|_| Error::Invalid("lost stop reason"))?;
            }
            _ => p.check()?,
        }
        if !matches!(bug, Bug::MissingReports) {
            p.advance(2);
        }
        Ok::<_, Error>(())
    });
    if matches!(bug, Bug::DuplicateStage) {
        phases.run(0, |p| p.check().map_err(Error::from))?;
    }
    if matches!(bug, Bug::MissingHandoff) {
        return result;
    }
    // ForgottenRequiredStage looks exactly like an intentional unused branch.
    result.finish_phase(phases)
}

/// Same work with and without the final partial-batch cancellation check.
pub fn final_batch(
    pulse: &dyn Pulse,
    request_stop: impl FnOnce(),
    finish: bool,
) -> Result<(), Error> {
    pulse.check()?;
    let mut paced = pulse.paced(64);
    paced.step(2)?;
    request_stop();
    if finish {
        paced.finish()?;
    }
    Ok(())
}
