//! Actual zenpng encode paths, deterministic input, byte parity and cancellation.
use enough::Unstoppable;
#[cfg(feature = "progress")]
use how_far_along::{prelude::*, *};
use imgref::ImgRef;
use rgb::Rgb;
use zenpng::{Compression, EncodeConfig};

fn pixels(size: usize) -> Vec<Rgb<u8>> {
    (0..size * size)
        .map(|i| {
            Rgb::new(
                (i * 17) as u8,
                ((i / size) * 13) as u8,
                ((i ^ (i / size)) * 7) as u8,
            )
        })
        .collect()
}
fn old(data: &[Rgb<u8>], size: usize, config: &EncodeConfig) -> Vec<u8> {
    zenpng::encode_rgb8(
        ImgRef::new(data, size, size),
        None,
        config,
        &Unstoppable,
        &Unstoppable,
    )
    .unwrap()
}
#[cfg(feature = "progress")]
fn encode(
    data: &[Rgb<u8>],
    size: usize,
    config: &EncodeConfig,
    p: &dyn Pulse,
) -> Result<Vec<u8>, whereat::At<zenpng::PngError>> {
    zenpng::encode_rgb8_with_pulse(ImgRef::new(data, size, size), None, config, p, &Unstoppable)
}
#[cfg(feature = "progress")]
fn is_stop(error: &whereat::At<zenpng::PngError>) -> bool {
    matches!(error.error(), zenpng::PngError::Stopped(_))
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("old");
    let iterations: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let size: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(128);
    let data = pixels(size);
    let config = EncodeConfig::default().with_compression(Compression::Fast);
    let baseline = old(&data, size, &config);
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let output = match mode {
            "old" => old(&data, size, &config),
            #[cfg(feature = "progress")]
            "nopulse" => encode(&data, size, &config, &NoPulse).unwrap(),
            #[cfg(feature = "progress")]
            "tree" => {
                let tree = PulseTree::new(Phase::new("png", Total::Unknown), Unstoppable);
                let result = encode(&data, size, &config, &tree);
                tree.complete_classified(result, is_stop).unwrap()
            }
            #[cfg(feature = "progress")]
            "callback" => {
                let pulse = FnPulse::new("png", |_| Ok(()));
                let result = encode(&data, size, &config, &pulse);
                pulse.complete_classified(result, is_stop).unwrap()
            }
            _ => panic!("unknown mode"),
        };
        assert_eq!(output, baseline);
        std::hint::black_box(output);
    }
    println!(
        "{{\"mode\":\"{mode}\",\"iterations\":{iterations},\"size\":{size},\"elapsed_ns\":{},\"png_bytes\":{}}}",
        start.elapsed().as_nanos(),
        baseline.len()
    );
}
#[cfg(all(test, feature = "progress"))]
mod tests {
    use super::*;
    #[test]
    fn serial_and_parallel_output_and_counts_match() {
        for parallel in [false, true] {
            let size = 64;
            let data = pixels(size);
            let mut config = EncodeConfig::default().with_compression(Compression::Fast);
            config.parallel = parallel;
            let baseline = old(&data, size, &config);
            let tree = PulseTree::new(Phase::new("png", Total::Unknown), Unstoppable);
            let observer = tree.observer();
            let result = encode(&data, size, &config, &tree);
            assert_eq!(observer.snapshot().status, Status::Running);
            assert_eq!(tree.complete_classified(result, is_stop).unwrap(), baseline);
            let snapshot = observer.snapshot();
            assert_eq!(snapshot.children.len(), 4);
            assert!(
                snapshot.children[0].completed > 0,
                "real verified strategy count"
            );
            assert_eq!(snapshot.fraction(), Some(1.0));
        }
    }
    #[test]
    fn callback_cancels_after_real_encoder_work() {
        let size = 64;
        let data = pixels(size);
        let config = EncodeConfig::default().with_compression(Compression::Fast);
        let pulse = FnPulse::new("png", |cx| {
            let snapshot = cx.snapshot();
            if snapshot.children.first().is_some_and(|s| s.completed > 0) {
                Err(StopReason::Cancelled)
            } else {
                Ok(())
            }
        });
        let observer = pulse.observer();
        let result = zenpng::encode_rgb8_with_pulse(
            ImgRef::new(&data, size, size),
            None,
            &config,
            &pulse,
            &Unstoppable,
        );
        let error = pulse.complete_classified(result, is_stop).unwrap_err();
        assert!(matches!(
            error.error(),
            zenpng::PngError::Stopped(StopReason::Cancelled)
        ));
        let snapshot = observer.snapshot();
        assert!(snapshot.children[0].completed > 0);
        assert_eq!(
            snapshot.children[0].status,
            Status::Finished(Outcome::Cancelled)
        );
        assert_eq!(
            snapshot.children[1].status,
            Status::Finished(Outcome::NotRun)
        );
    }
    #[test]
    fn enabling_progress_preserves_the_old_cancellation_api() {
        let size = 64;
        let data = pixels(size);
        struct Cancel;
        impl Stop for Cancel {
            fn check(&self) -> Result<(), StopReason> {
                Err(StopReason::Cancelled)
            }
        }
        let result = zenpng::encode_rgb8(
            ImgRef::new(&data, size, size),
            None,
            &EncodeConfig::default(),
            &Cancel,
            &Unstoppable,
        );
        assert!(matches!(
            result.unwrap_err().error(),
            zenpng::PngError::Stopped(StopReason::Cancelled)
        ));
    }
    #[test]
    fn padded_input_nests_in_a_caller_plan_without_changing_encoded_bytes() {
        let size = 31;
        let data = pixels(size);
        let stride = size + 7;
        let mut padded = vec![Rgb::new(0xA5, 0xA5, 0xA5); stride * size];
        for y in 0..size {
            padded[y * stride..y * stride + size].copy_from_slice(&data[y * size..(y + 1) * size]);
        }
        let config = EncodeConfig::default().with_compression(Compression::Fast);
        let expected = old(&data, size, &config);
        let root = PulseTree::new(Phase::new("pipeline", Total::Unknown), Unstoppable);
        let observer = root.observer();
        let result = Stages::new(&root, &[PhaseSpec::new("encode", 1, Total::Unknown)])
            .complete_with_classified(is_stop, |stages| {
                stages.run_classified(is_stop, |stage| {
                    zenpng::encode_rgb8_with_pulse(
                        ImgRef::new_stride(&padded, size, size, stride),
                        None,
                        &config,
                        stage,
                        &Unstoppable,
                    )
                })
            });
        assert_eq!(observer.snapshot().status, Status::Running);
        assert_eq!(root.complete_classified(result, is_stop).unwrap(), expected);
        let snapshot = observer.snapshot();
        assert_eq!(snapshot.status, Status::Finished(Outcome::Succeeded));
        assert_eq!(snapshot.children[0].children.len(), 4);
    }
}
