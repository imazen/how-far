//! Stages a codec operation could declare, from where and when it checks
//! its `Stop`: each case runs once under a how-far-really span that times
//! every check, then prints the `SuggestedStages` finding and the checks'
//! time grouped by crate, in the order the crates ran.
//!
//! usage: stage-suggestions [--json DIR] [CASE...]   (`list` lists cases)

mod cases;
mod inputs;

use enough::{Stop, StopReason, Unstoppable};
use how_far::Outcome;
use how_far_really::diagnostics::{Kind, Options};
use how_far_really::profile::{Instrumented, Profiler, SiteStats, Span, SpanKind, StdClock};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// A stop whose span covers only the measured operation: it starts at the
/// case's `begin` (or else at the first check) and ends at its `end`, so
/// neither input preparation nor dropping the buffers is measured.
struct Lazy<'p> {
    profiler: &'p Profiler,
    task: &'static str,
    started: OnceLock<Instrumented<Unstoppable>>,
    span: Mutex<Option<Span>>,
}

impl Lazy<'_> {
    fn begin(&self) -> &Instrumented<Unstoppable> {
        self.started.get_or_init(|| {
            let span = self.profiler.span(None, self.task, SpanKind::Work);
            let stop = span.instrument(Unstoppable);
            *self.span.lock().unwrap() = Some(span);
            stop
        })
    }

    /// Finish the span with `outcome` if it is still open.
    fn end(&self, outcome: Outcome) {
        if let Some(span) = self.span.lock().unwrap().take() {
            span.finish(outcome);
        }
    }
}

impl Stop for Lazy<'_> {
    #[track_caller]
    fn check(&self) -> Result<(), StopReason> {
        self.begin().check()
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json = match args.iter().position(|a| a == "--json") {
        Some(i) => {
            let dir = args.remove(i + 1);
            args.remove(i);
            std::fs::create_dir_all(&dir).unwrap();
            Some(dir)
        }
        None => None,
    };
    let registry = cases::all();
    if args.first().is_some_and(|a| a == "list") {
        for case in &registry {
            println!("{:32} [{}] {}", case.name, case.codec, case.blurb);
        }
        return;
    }
    for case in registry
        .iter()
        .filter(|c| args.is_empty() || args.iter().any(|a| a == c.name))
    {
        println!("=== {} [{}]\n  {}", case.name, case.codec, case.blurb);
        let profiler = Profiler::new(StdClock::new(), 4);
        profiler.metadata("case", case.name);
        profiler.metadata("codec", case.codec);
        profiler.metadata("operation", case.blurb);
        let stop = Lazy {
            profiler: &profiler,
            task: case.name,
            started: OnceLock::new(),
            span: Mutex::new(None),
        };
        let result = (case.run)(
            &stop,
            &|| {
                stop.begin();
            },
            &|| stop.end(Outcome::Succeeded),
        );
        if stop.started.get().is_none() {
            println!("  no checks and no begin: nothing measured\n");
            continue;
        }
        // A case that failed before `end` finishes here.
        stop.end(Outcome::Failed);
        match &result {
            Ok(message) => println!("  ok: {message}"),
            Err(error) => println!("  FAILED: {error}"),
        }
        let trace = profiler.snapshot();
        if let Some(dir) = &json {
            let mut out = String::new();
            trace.write_json(&mut out).unwrap();
            std::fs::write(format!("{dir}/{}.json", case.name), out).unwrap();
        }
        let span = &trace.spans[0];
        let wall = span.elapsed();
        // Each location's time is the intervals ending at its calls; what
        // ran after the last timed call ends at none.
        let last = span
            .stats
            .sites
            .iter()
            .filter_map(|s| s.active.map(|a| a.1))
            .max();
        let first = span
            .stats
            .sites
            .iter()
            .filter_map(|s| s.active.map(|a| a.0))
            .min();
        println!(
            "  wall {:.1} ms, {} checks at {} locations; first call at {:.1} ms, nothing timed for the last {:.1} ms",
            ms(wall),
            span.stats.checks,
            span.stats.sites.len(),
            ms(first.unwrap_or(span.end).saturating_sub(span.start)),
            ms(span.end.saturating_sub(last.unwrap_or(span.start)))
        );
        let mut options = Options::default();
        options.minimum_stage_wall = Duration::from_millis(5);
        for finding in trace.diagnose(&options) {
            if finding.kind == Kind::SuggestedStages {
                print!("  {finding}");
                println!();
            }
        }
        by_crate(&span.stats.sites, span.start, wall);
        println!();
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// Each crate's intervals (those ending at its calls) as a share of wall
/// time, and when it was called, then its busiest locations, crates in the
/// order they were first called.
fn by_crate(sites: &[SiteStats], start: Duration, total: Duration) {
    struct Crate<'a> {
        name: &'a str,
        time: Duration,
        checks: u64,
        first: Duration,
        last: Duration,
        sites: Vec<&'a SiteStats>,
    }
    let mut crates: Vec<Crate> = Vec::new();
    for site in sites {
        let Some((first, last)) = site.active else {
            continue;
        };
        let name = package_of(site.file).0;
        let name = if name.is_empty() {
            "(this harness)"
        } else {
            name
        };
        match crates.iter_mut().find(|c| c.name == name) {
            Some(c) => {
                c.time += site.time;
                c.checks += site.checks;
                c.first = c.first.min(first);
                c.last = c.last.max(last);
                c.sites.push(site);
            }
            None => crates.push(Crate {
                name,
                time: site.time,
                checks: site.checks,
                first,
                last,
                sites: vec![site],
            }),
        }
    }
    crates.sort_by_key(|c| c.first);
    let share = |t: Duration| 100.0 * t.as_secs_f64() / total.as_secs_f64().max(1e-12);
    for c in &mut crates {
        println!(
            "  {:<14} {:5.1}%  {:8.1} to {:8.1} ms  {:>9} checks",
            c.name,
            share(c.time),
            ms(c.first.saturating_sub(start)),
            ms(c.last.saturating_sub(start)),
            c.checks
        );
        c.sites.sort_by_key(|s| std::cmp::Reverse(s.time));
        for s in c.sites.iter().take(4) {
            let (first, last) = s.active.unwrap();
            println!(
                "      {:5.1}%  {:8.1} to {:8.1} ms  {:>9} checks  {}:{}",
                share(s.time),
                ms(first.saturating_sub(start)),
                ms(last.saturating_sub(start)),
                s.checks,
                package_of(s.file).1,
                s.line
            );
        }
    }
}

/// The package a source path belongs to, and the path inside it: the
/// directory before the last `src`, `tests`, `benches` or `examples`
/// directory, without a `-<version>` suffix, or for a cargo git checkout
/// the repository directory without its hash.
fn package_of(file: &str) -> (&str, &str) {
    let parts: Vec<&str> = file.split(['/', '\\']).collect();
    let Some(at) = parts
        .iter()
        .rposition(|p| matches!(*p, "src" | "tests" | "benches" | "examples"))
    else {
        return ("", file);
    };
    // The byte offset of `parts[at]`: each earlier part and its separator.
    let offset: usize = parts[..at].iter().map(|p| p.len() + 1).sum();
    let inner = &file[offset..];
    if at == 0 {
        return ("", inner);
    }
    let mut name = parts[at - 1];
    if name.len() >= 7 && name.bytes().all(|b| b.is_ascii_hexdigit()) && at >= 2 {
        name = parts[at - 2]
            .rsplit_once('-')
            .map_or(parts[at - 2], |(repo, _)| repo);
    }
    let cut = name
        .as_bytes()
        .windows(2)
        .position(|w| w[0] == b'-' && w[1].is_ascii_digit());
    (cut.map_or(name, |i| &name[..i]), inner)
}

#[cfg(test)]
mod tests {
    use super::package_of;

    #[test]
    fn packages_come_from_the_directory_above_src() {
        for (file, expected) in [
            (
                "/r/index.crates.io-1949cf8c6b5b557f/zenflate-0.3.6/src/compress/mod.rs",
                ("zenflate", "src/compress/mod.rs"),
            ),
            ("/r/x264-sys-0.2.1/src/lib.rs", ("x264-sys", "src/lib.rs")),
            (
                "/g/checkouts/zenpng-1a2b3c4d5e6f7a8b/9f8e7d6/src/lib.rs",
                ("zenpng", "src/lib.rs"),
            ),
            (
                "/home/u/work/zen/zenjpeg/zenjpeg/src/decode.rs",
                ("zenjpeg", "src/decode.rs"),
            ),
            ("src/main.rs", ("", "src/main.rs")),
            ("build.rs", ("", "build.rs")),
            (
                r"C:\work\zenwebp\src\lossy.rs",
                ("zenwebp", r"src\lossy.rs"),
            ),
        ] {
            assert_eq!(package_of(file), expected, "{file}");
        }
    }
}
