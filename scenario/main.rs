//! zenoh#2825 scenario: an in-process router and a peer session connected over
//! TCP loopback. The peer declares then undeclares liveliness tokens, each on a
//! key never used before, as a process creating and destroying ROS 2 nodes does.
//!
//! Usage: `scenario <count> <port>`
//!
//! Prints one progress line per 5,000 tokens, `FROZEN` when no token is declared
//! for 10 s, checks that a `put` from the peer still reaches a subscriber on the
//! router (retried for up to 60 s), then prints a `RESULT` line.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use zenoh::{Config, Wait};

const BATCH: u64 = 5_000;
const STALL: Duration = Duration::from_secs(10);
const CANARY: Duration = Duration::from_secs(60);

fn config(mode: &str, port: u16) -> Config {
    let mut config = Config::default();
    config.insert_json5("mode", &format!("\"{mode}\"")).unwrap();
    config
        .insert_json5("scouting/multicast/enabled", "false")
        .unwrap();
    config
        .insert_json5("scouting/gossip/enabled", "false")
        .unwrap();
    let endpoint = format!("[\"tcp/127.0.0.1:{port}\"]");
    if mode == "router" {
        config.insert_json5("listen/endpoints", &endpoint).unwrap();
    } else {
        config.insert_json5("connect/endpoints", &endpoint).unwrap();
        config.insert_json5("listen/endpoints", "[]").unwrap();
    }
    config
}

fn median(latencies: &mut [Duration]) -> Duration {
    latencies.sort();
    latencies[latencies.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count: u64 = args.get(1).map_or(100_000, |a| a.parse().unwrap());
    let port: u16 = args.get(2).map_or(17447, |a| a.parse().unwrap());

    let router = zenoh::open(config("router", port)).wait().unwrap();
    let canary = router.declare_subscriber("repro/canary").wait().unwrap();
    let peer = zenoh::open(config("peer", port)).wait().unwrap();
    std::thread::sleep(Duration::from_millis(500));

    let start = Instant::now();
    let declared = Arc::new(AtomicU64::new(0));
    let worker = {
        let declared = declared.clone();
        let peer = peer.clone();
        std::thread::spawn(move || {
            let mut latencies = Vec::with_capacity(BATCH as usize);
            for i in 0..count {
                let key = format!("repro/churn/k{i:09}/leaf");
                let t0 = Instant::now();
                let token = peer.liveliness().declare_token(key).wait().unwrap();
                latencies.push(t0.elapsed());
                token.undeclare().wait().unwrap();
                declared.store(i + 1, Ordering::Relaxed);
                if (i + 1) % BATCH == 0 {
                    println!(
                        "declared {:>6} tokens, median declare {:>9.1} us, {:>6.1} s elapsed",
                        i + 1,
                        median(&mut latencies).as_secs_f64() * 1e6,
                        start.elapsed().as_secs_f64(),
                    );
                    latencies.clear();
                }
            }
        })
    };

    let mut last = 0;
    let mut last_change = Instant::now();
    while !worker.is_finished() {
        std::thread::sleep(Duration::from_millis(200));
        let now = declared.load(Ordering::Relaxed);
        if now != last {
            last = now;
            last_change = Instant::now();
        } else if last_change.elapsed() > STALL {
            println!(
                "FROZEN: no token declared for {} s after {now}",
                STALL.as_secs()
            );
            break;
        }
    }
    let frozen = !worker.is_finished();
    let elapsed = start.elapsed();

    let canary_start = Instant::now();
    std::thread::spawn(move || {
        while canary_start.elapsed() < CANARY {
            peer.put("repro/canary", "ping").wait().unwrap();
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    let delivered = canary.recv_timeout(CANARY).ok().flatten().is_some();
    println!(
        "canary put from peer {} the router after {:.1} s",
        if delivered {
            "reached"
        } else {
            "did not reach"
        },
        canary_start.elapsed().as_secs_f64()
    );
    println!(
        "RESULT frozen={frozen} declared={} canary_delivered={delivered} elapsed_secs={:.1}",
        declared.load(Ordering::Relaxed),
        elapsed.as_secs_f64()
    );
    std::process::exit(if frozen || !delivered { 2 } else { 0 });
}
