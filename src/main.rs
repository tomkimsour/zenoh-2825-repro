//! Builds the zenoh#2825 scenario against `zenoh-before` and `zenoh-after`, runs both,
//! and reports whether the bug shows before the fix and is gone after it.
//!
//! Usage: `cargo run --release -- [count]` (default 100000 tokens).

use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::{Duration, Instant},
};

use zenoh_2825_repro::{parse_result, verdict, Outcome, Verdict};

const VARIANTS: [&str; 2] = ["before", "after"];
const HARD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Builds `scenario-<variant>` in release mode and returns the path of its binary.
fn build(root: &Path, variant: &str) -> PathBuf {
    let crate_dir = root.join(format!("scenario-{variant}"));
    let status = Command::new("cargo")
        .args(["build", "--release", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        .current_dir(root)
        .status()
        .expect("failed to run cargo");
    assert!(status.success(), "building scenario-{variant} failed");
    crate_dir.join(format!("target/release/scenario-{variant}"))
}

/// Returns a TCP port on the loopback interface that is free right now.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .expect("no free loopback port")
}

/// Runs one scenario binary, echoing its output, and returns what it reported.
fn run(binary: &Path, variant: &str, count: u64) -> Outcome {
    let start = Instant::now();
    let mut child = Command::new(binary)
        .arg(count.to_string())
        .arg(free_port().to_string())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start the scenario");
    let stdout = child.stdout.take().unwrap();
    let label = variant.to_string();
    let reader = std::thread::spawn(move || {
        let mut outcome = None;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("[{label}] {line}");
            outcome = outcome.or_else(|| parse_result(&line));
        }
        outcome
    });
    while child
        .try_wait()
        .expect("failed to wait for the scenario")
        .is_none()
    {
        if start.elapsed() > HARD_TIMEOUT {
            println!("[{variant}] killed after {} s", HARD_TIMEOUT.as_secs());
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = child.wait();
    reader.join().unwrap().unwrap_or(Outcome {
        frozen: true,
        declared: 0,
        canary_delivered: false,
        elapsed_secs: start.elapsed().as_secs_f64(),
    })
}

/// Returns the short commit id checked out in `zenoh-<variant>`.
fn commit(root: &Path, variant: &str) -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root.join(format!("zenoh-{variant}")))
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn main() -> ExitCode {
    let count: u64 = std::env::args()
        .nth(1)
        .map_or(100_000, |a| a.parse().expect("count must be a number"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let binaries: Vec<PathBuf> = VARIANTS.iter().map(|v| build(root, v)).collect();
    let outcomes: Vec<Outcome> = VARIANTS
        .iter()
        .zip(&binaries)
        .map(|(variant, binary)| run(binary, variant, count))
        .collect();

    println!();
    println!(
        "{:<7} {:<10} {:>7} {:>9} {:>7} {:>9}",
        "zenoh", "commit", "frozen", "declared", "canary", "time"
    );
    for (variant, outcome) in VARIANTS.iter().zip(&outcomes) {
        println!(
            "{:<7} {:<10} {:>7} {:>9} {:>7} {:>7.1} s",
            variant,
            commit(root, variant),
            if outcome.frozen { "yes" } else { "no" },
            outcome.declared,
            if outcome.canary_delivered {
                "ok"
            } else {
                "lost"
            },
            outcome.elapsed_secs,
        );
    }

    let result = verdict(&outcomes[0], &outcomes[1], count);
    println!();
    match result {
        Verdict::ReproducedAndFixed => println!(
            "zenoh#2825 reproduced on zenoh-before and fixed on zenoh-after ({count} tokens)"
        ),
        Verdict::NotReproduced => println!(
            "zenoh#2825 not reproduced: zenoh-before completed {count} tokens (use more than 65535)"
        ),
        Verdict::NotFixed => println!("zenoh#2825 reproduced, but zenoh-after did not complete"),
    }
    if result == Verdict::ReproducedAndFixed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
