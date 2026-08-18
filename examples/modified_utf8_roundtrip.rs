//! Exercise Java Modified UTF-8 encoding without starting a JVM.

use jvmti_bindings::mutf8;

fn main() {
    let input = "NUL=\0, supplementary=🚀, accents=Zürich";
    let encoded = mutf8::encode(input);
    let decoded = mutf8::decode(&encoded).expect("library-produced MUTF-8 must decode");
    assert_eq!(decoded, input);
    println!("utf8_bytes={} mutf8_bytes={}", input.len(), encoded.len());
}
