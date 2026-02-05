use criterion::{criterion_group, criterion_main, Criterion};
use jvmti_bindings::classfile::ClassFile;

fn build_min_class() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xCAFEBABE_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&52_u16.to_be_bytes());

    // constant pool count = 5
    bytes.extend_from_slice(&5_u16.to_be_bytes());

    // 1: Utf8 "Test"
    bytes.push(1);
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(b"Test");

    // 2: Utf8 "java/lang/Object"
    bytes.push(1);
    bytes.extend_from_slice(&16_u16.to_be_bytes());
    bytes.extend_from_slice(b"java/lang/Object");

    // 3: Class #1
    bytes.push(7);
    bytes.extend_from_slice(&1_u16.to_be_bytes());

    // 4: Class #2
    bytes.push(7);
    bytes.extend_from_slice(&2_u16.to_be_bytes());

    // access_flags, this_class, super_class
    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&3_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());

    // interfaces, fields, methods, attributes
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());

    bytes
}

fn bench_classfile_parse(c: &mut Criterion) {
    let bytes = build_min_class();
    c.bench_function("classfile_parse_min", |b| {
        b.iter(|| {
            let _ = ClassFile::parse(&bytes).unwrap();
        })
    });
}

criterion_group!(benches, bench_classfile_parse);
criterion_main!(benches);
