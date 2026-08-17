use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct PreparedInput {
    root: PathBuf,
    temporary: bool,
    extraction_time: Duration,
}

impl PreparedInput {
    fn prepare(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.is_dir() {
            return Ok(Self {
                root: path.to_path_buf(),
                temporary: false,
                extraction_time: Duration::ZERO,
            });
        }

        let archive = path.canonicalize()?;
        let root = unique_temp_dir();
        fs::create_dir_all(&root)?;
        let start = Instant::now();
        let result = Command::new(jar_command())
            .current_dir(&root)
            .arg("xf")
            .arg(&archive)
            .status();
        let extraction_time = start.elapsed();

        match result {
            Ok(status) if status.success() => Ok(Self {
                root,
                temporary: true,
                extraction_time,
            }),
            Ok(status) => {
                let _ = fs::remove_dir_all(&root);
                Err(format!("jar extraction failed with status {status}").into())
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&root);
                Err(format!("failed to execute the JDK jar tool: {error}").into())
            }
        }
    }
}

impl Drop for PreparedInput {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn jar_command() -> PathBuf {
    if let Some(java_home) = env::var_os("JAVA_HOME") {
        let executable = if cfg!(windows) { "jar.exe" } else { "jar" };
        let candidate = PathBuf::from(java_home).join("bin").join(executable);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(if cfg!(windows) { "jar.exe" } else { "jar" })
}

fn unique_temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("jvmti-jar-bench-{}-{nonce}", std::process::id()))
}

fn class_files(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut pending = vec![root.to_path_buf()];
    let mut classes = Vec::new();
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file()
                && entry.path().extension().is_some_and(|ext| ext == "class")
            {
                classes.push(entry.path());
            }
        }
    }
    classes.sort_unstable();
    Ok(classes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = env::args()
        .nth(1)
        .expect("usage: jar_parse_bench JAR_OR_CLASS_DIRECTORY");
    let prepared = PreparedInput::prepare(Path::new(&input))?;
    let classes = class_files(&prepared.root)?;

    let mut total_bytes: u64 = 0;
    let mut parsed: u64 = 0;
    let mut failed: u64 = 0;
    let start = Instant::now();
    for path in &classes {
        let bytes = fs::read(path)?;
        total_bytes += bytes.len() as u64;
        match jvmti_bindings::classfile::ClassFile::parse(&bytes) {
            Ok(_) => parsed += 1,
            Err(_) => failed += 1,
        }
    }
    let parse_time = start.elapsed();
    let total_time = prepared.extraction_time + parse_time;

    let parse_seconds = parse_time.as_secs_f64();
    let megabytes = total_bytes as f64 / (1024.0 * 1024.0);
    let ns_per_class = if parsed > 0 {
        parse_time.as_nanos() as f64 / parsed as f64
    } else {
        0.0
    };
    let mb_per_second = if parse_seconds > 0.0 {
        megabytes / parse_seconds
    } else {
        0.0
    };

    println!("input={input}");
    println!("class_files={}", classes.len());
    println!("parsed_ok={parsed} failed={failed}");
    println!("total_mb={megabytes:.3}");
    println!(
        "extract_time_ms={:.3}",
        prepared.extraction_time.as_secs_f64() * 1_000.0
    );
    println!("parse_time_ms={:.3}", parse_seconds * 1_000.0);
    println!("total_time_ms={:.3}", total_time.as_secs_f64() * 1_000.0);
    println!("ns_per_class={ns_per_class:.1}");
    println!("parse_mb_per_s={mb_per_second:.2}");

    Ok(())
}
