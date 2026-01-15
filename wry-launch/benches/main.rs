use std::time::{Duration, Instant};

mod batching;
mod roundtrip;

struct BenchResult {
    name: String,
    iterations: u64,
    total_time: Duration,
    avg_time: Duration,
}

impl BenchResult {
    fn print(&self) {
        println!(
            "{:<50} {:>12} iters  {:?}/iter  {:>10.2} ms total",
            self.name,
            self.iterations,
            self.avg_time,
            self.total_time.as_secs_f64() * 1000.0
        );
    }
}

fn bench<F: Fn()>(name: &str, f: F) -> BenchResult {
    let warmup_iters = 10;
    for _ in 0..warmup_iters {
        f();
    }

    let iterations = 100u64;

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();

    let avg_time = elapsed.div_f64(iterations as f64);

    BenchResult {
        name: name.to_string(),
        iterations,
        total_time: elapsed,
        avg_time,
    }
}

fn main() {
    wry_launch::run_headless(|| async {
        println!("\n{:=<100}", "");
        println!("{:^100}", "wry-bindgen Benchmarks");
        println!("{:=<100}\n", "");

        println!(
            "{:<50} {:>12}        {:>10}        {:>10}",
            "Benchmark", "Iterations", "Avg Time", "Total Time"
        );
        println!("{:-<100}", "");

        let results = vec![
            bench("roundtrip/u32", roundtrip::bench_roundtrip_u32),
            bench("roundtrip/u64", roundtrip::bench_roundtrip_u64),
            bench("roundtrip/i32", roundtrip::bench_roundtrip_i32),
            bench("roundtrip/i64", roundtrip::bench_roundtrip_i64),
            bench("roundtrip/f32", roundtrip::bench_roundtrip_f32),
            bench("roundtrip/f64", roundtrip::bench_roundtrip_f64),
            bench("roundtrip/bool", roundtrip::bench_roundtrip_bool),
            bench("roundtrip/string", roundtrip::bench_roundtrip_string),
            bench(
                "roundtrip/large-string",
                roundtrip::bench_roundtrip_large_string,
            ),
            bench(
                "roundtrip/option_some",
                roundtrip::bench_roundtrip_option_some,
            ),
            bench(
                "roundtrip/option_none",
                roundtrip::bench_roundtrip_option_none,
            ),
            bench("batch/add_1_calls", batching::bench_batch_add_1),
            bench("batch/add_100_calls", batching::bench_batch_add_100),
            bench(
                "batch/create_element_1_calls",
                batching::bench_batch_create_element_1,
            ),
            bench(
                "batch/create_element_100_calls",
                batching::bench_batch_create_element_100,
            ),
        ];

        for result in &results {
            result.print();
        }

        println!("{:=<100}\n", "");
    })
    .unwrap();
}
