//! Offline page-generation timing with the normal system allocator, not DHAT.
#[path = "main.rs"]
#[allow(dead_code)]
mod app;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: rustdl-page-bench <workload> <samples:1..1000>".into());
    }
    let samples: usize = args[1].parse()?;
    if !(1..=1000).contains(&samples) {
        return Err("samples must be 1..1000".into());
    }
    let workload = app::heap_workload::Workload::new(&args[0])?;
    let start = std::time::Instant::now();
    let expected = workload.run(1)?;
    let first_ns = start.elapsed().as_nanos();
    for _ in 0..5 {
        if workload.run(1)? != expected {
            return Err("unstable page checksum during warmup".into());
        }
    }
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = std::time::Instant::now();
        let actual = workload.run(1)?;
        let elapsed = start.elapsed().as_nanos();
        if actual != expected {
            return Err("page content changed during timing".into());
        }
        timings.push(elapsed);
    }
    timings.sort_unstable();
    println!(
        "{}",
        serde_json::json!({"workload":args[0],"samples":samples,"firstNs":first_ns,"medianNs":timings[samples/2],"p95Ns":timings[(samples*95).div_ceil(100).saturating_sub(1)],"minNs":timings[0],"maxNs":timings[samples-1],"checksum":expected})
    );
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("page bench: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
