use std::hint::black_box;
use std::time::{Duration, Instant};

use jvmti_bindings::classfile::ClassFile;

const DEFAULT_WARMUP: Duration = Duration::from_millis(250);
const DEFAULT_MEASURE: Duration = Duration::from_secs(2);

fn build_min_class() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xCAFEBABE_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&52_u16.to_be_bytes());

    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(b"Test");
    bytes.push(1);
    bytes.extend_from_slice(&16_u16.to_be_bytes());
    bytes.extend_from_slice(b"java/lang/Object");
    bytes.push(7);
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.push(7);
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&3_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}

fn run_for(bytes: &[u8], duration: Duration) -> (u64, Duration) {
    let start = Instant::now();
    let mut iterations = 0_u64;
    while start.elapsed() < duration {
        let parsed = ClassFile::parse(black_box(bytes)).expect("valid benchmark class");
        black_box(parsed);
        iterations += 1;
    }
    (iterations, start.elapsed())
}

fn main() {
    let bytes = build_min_class();
    let _ = run_for(&bytes, DEFAULT_WARMUP);
    let (iterations, elapsed) = run_for(&bytes, DEFAULT_MEASURE);
    let ns_per_iteration = elapsed.as_nanos() as f64 / iterations as f64;
    let iterations_per_second = iterations as f64 / elapsed.as_secs_f64();

    println!("benchmark=classfile_parse_min");
    println!("iterations={iterations}");
    println!("elapsed_ms={:.3}", elapsed.as_secs_f64() * 1_000.0);
    println!("ns_per_iteration={ns_per_iteration:.1}");
    println!("iterations_per_second={iterations_per_second:.1}");
}
