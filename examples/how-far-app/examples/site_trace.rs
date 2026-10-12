//! Real tracker states on an illustrative photo-export timeline, not a codec benchmark.
use how_far_along::{prelude::*, *};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Default)]
struct Trace {
    events: Vec<String>,
    frames: Vec<String>,
    time_ms: u64,
    smoother: Option<ProgressSmoother>,
    findings: Vec<String>,
    active_finding: Option<usize>,
}
impl Trace {
    fn event(&mut self, observer: &Observer, title: &str, detail: &str, code: &str) {
        // Authored ASCII text uses only JSON-compatible Rust string escapes.
        assert!(title.is_ascii() && detail.is_ascii() && code.is_ascii());
        self.events.push(format!(
            "{{\"title\":{title:?},\"detail\":{detail:?},\"code\":{code:?},\"frame\":{}}}",
            self.frames.len()
        ));
        self.frame(observer, 0);
    }
    fn frame(&mut self, observer: &Observer, delay_ms: u64) {
        self.time_ms += delay_ms;
        let mut snapshot = String::new();
        observer.snapshot().write_json(&mut snapshot).unwrap();
        let displayed = self.smoother.as_mut().and_then(|s| {
            s.display_fraction(
                &observer.snapshot(),
                std::time::Duration::from_millis(self.time_ms),
            )
        });
        let displayed = displayed.map_or_else(|| "null".into(), |value| value.to_string());
        let finding = self
            .active_finding
            .map_or_else(|| "null".into(), |i| i.to_string());
        self.frames.push(format!(
            "{{\"at_ms\":{},\"event\":{},\"display_fraction\":{displayed},\"finding\":{finding},\"snapshot\":{snapshot}}}",
            self.time_ms,
            self.events.len() - 1
        ));
    }
    fn json(self) -> String {
        format!(
            "{{\"events\":[{}],\"frames\":[{}],\"findings\":[{}]}}",
            self.events.join(","),
            self.frames.join(","),
            self.findings.join(",")
        )
    }
}
fn export(cancel: bool) -> String {
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let root = FnPulse::new("Export a photo", move |_| {
        if signal.load(Ordering::Relaxed) {
            Err(StopReason::Cancelled)
        } else {
            Ok(())
        }
    });
    let observer = root.observer();
    let mut trace = Trace::default();
    let (mut stages, [decode, resize, encode]) = Stages::define(
        &root,
        [
            PhaseSpec::new("Decode scanlines", 4, Total::Exact(2048)),
            PhaseSpec::new("Resize to 1600 px", 3, Total::Exact(1536)),
            PhaseSpec::new("Encode WebP tiles", 3, Total::Exact(256)),
        ],
    );
    trace.event(&observer, "Plan the photo export", "Each operation gets a named handle and its own units. Weights describe its share of the whole job.", "let (mut stages, [decode, resize, encode]) = Stages::define(pulse, specs);");
    let stage = stages.begin(decode);
    trace.event(&observer, "Decode scanlines", "Report batches of 32 completed scanlines. The caller can redraw between reports without changing your function.", "let stage = stages.begin(decode);\ndecode_scanlines(input, &stage)");
    for _ in 0..64 {
        stage.step(32).unwrap();
        trace.frame(&observer, 28);
    }
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    trace.event(&observer, "Hand the decoded pixels forward", "The result finishes decoding. The next operation takes the decoded pixels and a different unit count.", "let pixels = decode_scanlines(input, &stage).finish_phase(stage)?;");
    let stage = stages.begin(resize);
    trace.event(&observer, "Resize to 1600 px", "Count completed output rows. The stage's weight keeps these units separate from scanlines and encoded tiles.", "let stage = stages.begin(resize);\nresize_image(&pixels, &stage)");
    let result = if cancel {
        for _ in 0..32 {
            stage.step(16).unwrap();
            trace.frame(&observer, 42);
        }
        cancelled.store(true, Ordering::Relaxed);
        let result = stage.check().finish_phase(stage);
        trace.event(&observer, "Cancellation reaches a checkpoint", "The returned stop finishes the active stage as Cancelled. Encoding becomes NotRun, without filling its progress bar.", "resize_image(&pixels, &stage).finish_phase(stage)?;");
        result
    } else {
        for _ in 0..64 {
            stage.step(24).unwrap();
            trace.frame(&observer, 42);
        }
        Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
        let stage = stages.begin(encode);
        trace.event(&observer, "Encode WebP tiles", "A third operation reports four tiles at a time. The same handle pattern works with another input and result type.", "let stage = stages.begin(encode);\nencode_webp(&resized, &stage)");
        for _ in 0..64 {
            stage.step(4).unwrap();
            trace.frame(&observer, 34);
        }
        Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
        trace.event(&observer, "All stages finished", "The bar is full, but the job is still Running. Progress is an observation; only the owner's result finishes the job.", "stages.complete(Ok(encoded))");
        trace.frame(&observer, 0);
        Ok(())
    };
    let result = stages.complete(result);
    let _ = root.complete(result);
    trace.event(
        &observer,
        "The caller finishes the job",
        "The final result sets the root outcome. The library leaves this decision with its caller.",
        "let result = export_photo(input, &root);\nroot.complete(result)",
    );
    assert_eq!(
        observer.snapshot().status,
        Status::Finished(if cancel {
            Outcome::Cancelled
        } else {
            Outcome::Succeeded
        })
    );
    trace.json()
}
fn fork() -> String {
    let root = PulseTree::new(
        Phase::new("Prepare a photo export", Total::Unknown),
        Unstoppable,
    );
    let observer = root.observer();
    let mut trace = Trace::default();
    let (mut stages, [analysis, manifest]) = Stages::define(
        &root,
        [
            PhaseSpec::new("Analyze the image", 4, Total::Unknown),
            PhaseSpec::new("Write export manifest", 1, Total::Exact(4)),
        ],
    );
    let stage = stages.begin(analysis);
    let [pixels, metadata] = stage.fork([
        PhaseSpec::new("Build luminance histogram", 3, Total::Exact(256)),
        PhaseSpec::new("Read color profile", 1, Total::Exact(12)),
    ]);
    trace.event(&observer, "Fork two different operations", "One branch examines image tiles; the other reads profile tags. Your threads or runtime schedule them.", "let [pixels, metadata] = stage.fork(specs);");
    trace.event(&observer, "Independent work, one progress tree", "Inside std::thread::scope, spawn both workers before joining either. Each owns its stage and returns its result. This trace illustrates one possible interleaving.", "let pixels_task = scope.spawn(move ||\n    analyze_tiles(&image, &pixels)\n        .finish_phase(pixels));\nlet metadata_task = scope.spawn(move ||\n    read_profile(&bytes, &metadata)\n        .finish_phase(metadata));");
    for _ in 0..12 {
        pixels.step(4).unwrap();
        trace.frame(&observer, 30);
        metadata.step(1).unwrap();
        trace.frame(&observer, 25);
    }
    Ok::<_, StopReason>(()).finish_phase(metadata).unwrap();
    trace.event(&observer, "The color profile is ready", "Finishing this branch leaves the histogram and its parent running. Different return types are fine.", "// The metadata worker has returned.\n// The histogram worker is still running.");
    for _ in 0..52 {
        pixels.step(4).unwrap();
        trace.frame(&observer, 55);
    }
    Ok::<_, StopReason>(()).finish_phase(pixels).unwrap();
    trace.event(&observer, "Join both results", "Wait for both workers before combining their results. Finishing the children does not finish their parent.", "let histogram = pixels_task.join().unwrap();\nlet profile = metadata_task.join().unwrap();\nlet combined_result = match (histogram, profile) {\n    (Ok(h), Ok(p)) => Ok((h, p)),\n    (Err(e), _) | (_, Err(e)) => Err(e),\n};");
    trace.frame(&observer, 0);
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    let stage = stages.begin(manifest);
    trace.event(
        &observer,
        "Write the export manifest",
        "Hand the combined result to the parent, then continue with the next sequential operation.",
        "combined_result.finish_phase(stage)?;\nlet stage = stages.begin(manifest);",
    );
    for _ in 0..4 {
        stage.step(1).unwrap();
        trace.frame(&observer, 100);
    }
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    let result = stages.complete(Ok::<_, StopReason>(()));
    root.complete(result).unwrap();
    trace.event(&observer, "Ready for export", "All branches have joined, the manifest is complete, and the caller has handed over the final result.", "let result = prepare_export(input, &root);\nroot.complete(result)");
    assert_eq!(
        observer.snapshot().status,
        Status::Finished(Outcome::Succeeded)
    );
    trace.json()
}
fn skipping() -> String {
    let root = PulseTree::new(
        Phase::new("Export without a watermark", Total::Unknown),
        Unstoppable,
    );
    let observer = root.observer();
    let mut trace = Trace::default();
    let (mut stages, [resize, _watermark, encode]) = Stages::define(
        &root,
        [
            PhaseSpec::new("Resize for the gallery", 4, Total::Exact(1024)),
            PhaseSpec::new("Apply optional watermark", 1, Total::Unknown),
            PhaseSpec::new("Encode gallery tiles", 5, Total::Exact(128)),
        ],
    );
    let stage = stages.begin(resize);
    trace.event(&observer, "A plan can include optional work", "Resizing advances the overall bar immediately. The optional watermark keeps its reserved 10.0% share, shown hatched until it is resolved.", "let (mut stages, [resize, watermark, encode]) = Stages::define(pulse, specs);");
    for _ in 0..64 {
        stage.step(16).unwrap();
        trace.frame(&observer, 35);
    }
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    stages.skip();
    trace.event(&observer, "Skip the optional watermark", "Skipped means intentionally unnecessary. It resolves this obligation without pretending any watermark work ran.", "if !options.watermark { stages.skip(); }");
    trace.frame(&observer, 0);
    let stage = stages.begin(encode);
    trace.event(&observer, "Continue with the next operation", "The next named handle still works. Skipped is different from NotRun after cancellation or failure.", "let stage = stages.begin(encode);\nencode_tiles(&image, &stage).finish_phase(stage)?;");
    for _ in 0..64 {
        stage.step(2).unwrap();
        trace.frame(&observer, 40);
    }
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    let result = stages.complete(Ok::<_, StopReason>(()));
    root.complete(result).unwrap();
    trace.event(&observer, "A successful export, with less work", "The job succeeded. Its snapshot retains the skipped stage, so the caller can explain the chosen path.", "root.complete(result)");
    assert_eq!(
        observer.snapshot().children[1].status,
        Status::Finished(Outcome::Skipped)
    );
    trace.json()
}
fn smoothing() -> String {
    let root = PulseTree::new(
        Phase::new("Upload image tiles", Total::Exact(120)),
        Unstoppable,
    );
    let observer = root.observer();
    let mut trace = Trace {
        smoother: Some(ProgressSmoother::new()),
        ..Trace::default()
    };
    root.step(0).unwrap();
    trace.event(&observer, "Reports arrive in batches", "The library reports ten uploaded tiles at a time. Compare the recorded fraction with the optional display estimate below.", "let mut smooth = ProgressSmoother::new();");
    for batch in 0..12 {
        if batch == 2 {
            trace.event(&observer, "The display learns the reporting pace", "The green bar comes from the Rust ProgressSmoother. It estimates between reports; the purple bar and unit count remain recorded facts.", "smooth.display_fraction(&snapshot, elapsed)");
        }
        for _ in 0..12 {
            trace.frame(&observer, 40);
        }
        root.step(10).unwrap();
        trace.frame(&observer, 0);
    }
    trace.event(&observer, "Counts and display meet at completion", "Smoothing does not finish the operation. Final outcomes and revised totals always come from the tracker.", "root.complete(result)");
    root.complete(Ok::<_, StopReason>(())).unwrap();
    trace.event(&observer, "A smooth display, unchanged library code", "Enable the smooth feature in how-far-along. Keep one smoother per observed tree; use recorded states for decisions.", "how-far-along = { version = \"0.1.0\", features = [\"smooth\"] }");
    trace.json()
}
// The same illustrative clock drives playback and diagnostic timing. These
// deliberately authored workloads demonstrate findings, not codec measurements.
#[derive(Clone, Default)]
struct DemoClock(Arc<AtomicU64>);
impl how_far_really::profile::Clock for DemoClock {
    fn now(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.0.load(Ordering::Relaxed))
    }
}
fn diagnostics() -> String {
    use how_far_really::{
        diagnostics::{DiagnosticPulse, Kind, Options},
        profile::Profiler,
    };
    let clock = DemoClock::default();
    let profiler = Profiler::new(clock.clone(), 128);
    profiler.set_report_timing(true);
    let root = DiagnosticPulse::new(
        PulseTree::new(
            Phase::new("Audit a gallery export", Total::Unknown),
            Unstoppable,
        ),
        &profiler,
    );
    let observer = root.observer();
    let mut trace = Trace::default();
    let (mut stages, [resize, encode, preview]) = Stages::define(
        &root,
        [
            PhaseSpec::new("Resize gallery rows", 3, Total::Exact(64)),
            PhaseSpec::new("Encode gallery tiles", 5, Total::Exact(64)),
            PhaseSpec::new("Encode preview tiles", 2, Total::Exact(16)),
        ],
    );
    let stage = stages.begin(resize);
    trace.event(&observer, "Check the reporting contract", "This deliberately faulty integration under-reports rows, double-counts tiles and forgets a result handoff. Diagnostics observe the ordinary library calls.", "let pulse = DiagnosticPulse::new(tree, &profiler);");
    for _ in 0..48 {
        clock.0.store(trace.time_ms + 30, Ordering::Relaxed);
        stage.step(1).unwrap();
        trace.frame(&observer, 30);
    }
    trace.event(&observer, "Only 48 of 64 rows reported", "The operation returns success before flushing its last batch. Success resolves the stage, but the count mismatch stays visible to diagnostics.", "resize_rows(input, &stage).finish_phase(stage)?;");
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    let stage = stages.begin(encode);
    trace.event(&observer, "Tiles get slower, then get counted twice", "The second half costs more per tile. An extra batch also pushes the count beyond its exact total. A capped bar must not hide that evidence.", "profiler.set_report_timing(true);\nstage.step(completed_tiles)?;");
    for tile in 0..80 {
        let delay = if tile < 32 { 12 } else { 72 };
        clock.0.store(trace.time_ms + delay, Ordering::Relaxed);
        stage.step(1).unwrap();
        trace.frame(&observer, delay);
    }
    Ok::<_, StopReason>(()).finish_phase(stage).unwrap();
    let stage = stages.begin(preview);
    trace.event(&observer, "A full counter still needs its result", "Preview encoding reports all 16 tiles, then deliberately drops its stage without a result handoff. The owner records Abandoned.", "encode_preview(input, &stage)?;\n// Missing: result.finish_phase(stage)");
    for _ in 0..16 {
        clock.0.store(trace.time_ms + 30, Ordering::Relaxed);
        stage.step(1).unwrap();
        trace.frame(&observer, 30);
    }
    drop(stage);
    let result = stages.complete(Ok::<_, StopReason>(()));
    root.complete(result).unwrap();
    profiler.operation_returned();
    let findings = profiler
        .snapshot()
        .with_progress(observer.snapshot())
        .diagnose(&Options::default());
    let uneven = findings
        .iter()
        .find(|f| f.kind == Kind::UnevenPace)
        .expect("uneven tile costs must be detected");
    let mismatches: Vec<_> = findings
        .iter()
        .filter(|f| f.kind == Kind::CountMismatch)
        .collect();
    assert_eq!(
        mismatches.len(),
        2,
        "both under- and over-reporting must be detected"
    );
    let abandoned = findings
        .iter()
        .find(|f| f.kind == Kind::AbandonedPhase)
        .expect("missing handoff must be detected");
    for finding in [uneven, mismatches[0], mismatches[1], abandoned] {
        assert!(finding.evidence.is_ascii() && finding.advice.is_ascii());
        trace.findings.push(format!(
            "{{\"kind\":\"{:?}\",\"evidence\":{:?},\"advice\":{:?}}}",
            finding.kind, finding.evidence, finding.advice
        ));
    }
    let explanations = [
        (
            "Uneven work needs better units",
            "Equal tile counts need not mean equal effort. Split distinct operations into weighted stages, or count a unit that tracks their cost. This example uses an illustrative clock.",
            "trace.diagnose(&Options::default())",
        ),
        (
            "Under-reporting survives success",
            "The tracker kept 48 completed rows against an exact total of 64. Flush the final batch before handing over the result.",
            "batch.finish()?;\nresult.finish_phase(stage)?;",
        ),
        (
            "Over-reporting is not extra completion",
            "Encoding reported 80 tiles against an exact total of 64. The progress bar caps at 100.0%, while diagnostics preserve the original counts.",
            "// Report each completed unit exactly once.\nstage.step(newly_completed)?;",
        ),
        (
            "Diagnostics expose the missing handoff",
            "The caller's success does not erase an abandoned preview. All four findings come from how-far-really, using the recorded counts and illustrative timing.",
            "result.finish_phase(stage)?;",
        ),
    ];
    for (index, (title, detail, code)) in explanations.into_iter().enumerate() {
        trace.active_finding = Some(index);
        trace.event(&observer, title, detail, code);
        // Time to read completed-run findings; work itself never pauses for narration.
        for _ in 0..60 {
            trace.frame(&observer, 50);
        }
    }
    trace.json()
}

fn main() {
    println!(
        "{{\"sequence\":{},\"cancel\":{},\"fork\":{},\"skip\":{},\"smooth\":{},\"diagnostics\":{}}}",
        export(false),
        export(true),
        fork(),
        skipping(),
        smoothing(),
        diagnostics()
    );
}
