//! Separate profiling executable; the production app allocator is untouched.
#[path = "main.rs"]
#[allow(dead_code)]
mod app;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::DirBuilderExt;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: rustdl-heap-profile <workload> <iterations:1..100>; see PERFORMANCE.md".into(),
        );
    }
    let iterations: usize = args[1].parse()?;
    if !(1..=100).contains(&iterations) {
        return Err("iterations must be between 1 and 100".into());
    }
    // Setup allocations are intentionally outside the profiling window.
    let workload = app::heap_workload::Workload::new(&args[0])?;
    let dir = std::path::Path::new("target/heap-profiles");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let output = dir.join(format!("{}-{}.json", args[0], std::process::id()));
    // Reserve without overwriting an existing file, with private permissions.
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&output)?;
    drop(file);
    let profiler = dhat::Profiler::builder()
        .file_name(&output)
        .trim_backtraces(Some(24))
        .build();
    let result = workload.run(iterations);
    let stats = dhat::HeapStats::get();
    drop(profiler); // Android kills do not run destructors; finish explicitly.
    let checksum = result?;
    // DHAT reports write failures on stderr; also verify a nonempty report.
    if std::fs::metadata(&output)?.len() == 0 {
        return Err("DHAT did not write a report".into());
    }
    println!(
        "workload={} iterations={iterations} checksum={checksum}",
        args[0]
    );
    println!(
        "total_bytes={} total_blocks={} peak_bytes={} end_bytes={}",
        stats.total_bytes, stats.total_blocks, stats.max_bytes, stats.curr_bytes
    );
    println!("report={}", output.display());
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("heap profile: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
