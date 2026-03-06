use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use xray_rs_tls::obfuscation::Salamander;

fn criterion_benchmark(c: &mut Criterion) {
    let mut salamander = Salamander::new_thread(b"average_password").unwrap();

    let in_data = [0u8; 1200];
    let mut out_data = [0u8; 2048];

    let in_data = &in_data[..];
    let out_data = &mut out_data[..];

    c.bench_function("salamander_obfuscate", |b| {
        b.iter(|| salamander.obfuscate(black_box(in_data), black_box(out_data)))
    });

    c.bench_function("salamander_deobfuscate", |b| {
        b.iter(|| salamander.deobfuscate(black_box(in_data), black_box(out_data)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
