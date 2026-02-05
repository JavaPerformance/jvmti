use std::env;
use std::fs::File;
use std::io::Read;
use std::time::Instant;

use zip::ZipArchive;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jar_path = env::args().nth(1).expect("usage: jar_parse_bench JAR_PATH");
    let file = File::open(&jar_path)?;
    let mut zip = ZipArchive::new(file)?;

    let mut total_bytes: u64 = 0;
    let mut parsed: u64 = 0;
    let mut failed: u64 = 0;
    let mut class_files: u64 = 0;

    let start = Instant::now();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name();
        if !name.ends_with(".class") {
            continue;
        }
        class_files += 1;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        total_bytes += bytes.len() as u64;
        match jvmti_bindings::classfile::ClassFile::parse(&bytes) {
            Ok(_) => parsed += 1,
            Err(_) => failed += 1,
        }
    }
    let dur = start.elapsed();

    let secs = dur.as_secs_f64();
    let mb = total_bytes as f64 / (1024.0 * 1024.0);
    let ns_per = if parsed > 0 {
        (dur.as_nanos() as f64) / (parsed as f64)
    } else {
        0.0
    };
    let mb_per_s = if secs > 0.0 { mb / secs } else { 0.0 };

    println!("jar_path={}", jar_path);
    println!("class_files={}", class_files);
    println!("parsed_ok={} failed={}", parsed, failed);
    println!("total_mb={:.3}", mb);
    println!("parse_time_ms={:.3}", secs * 1000.0);
    println!("ns_per_class={:.1}", ns_per);
    println!("mb_per_s={:.2}", mb_per_s);

    Ok(())
}
