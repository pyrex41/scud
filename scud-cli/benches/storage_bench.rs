use criterion::{black_box, criterion_group, criterion_main, Criterion};
use scud::models::{Phase, Task};
use scud::storage::Storage;
use std::collections::HashMap;
use tempfile::TempDir;

fn bench_load_all_vs_load_one(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
    storage.initialize().unwrap();

    // Create 50 phases with 100 tasks each (5000 tasks total)
    let mut tasks = HashMap::new();
    for i in 0..50 {
        let mut phase = Phase::new(format!("PHASE-{}", i));
        for j in 0..100 {
            phase.add_task(Task::new(
                format!("task-{}", j),
                format!("Task {}", j),
                "Description".to_string(),
            ));
        }
        tasks.insert(format!("PHASE-{}", i), phase);
    }
    storage.save_tasks(&tasks).unwrap();

    let mut group = c.benchmark_group("storage_operations");

    group.bench_function("load_all_phases_then_get_one", |b| {
        b.iter(|| {
            let all_tasks = storage.load_tasks().unwrap();
            black_box(all_tasks.get("PHASE-25").unwrap());
        })
    });

    group.bench_function("load_one_phase_directly", |b| {
        b.iter(|| {
            let phase = storage.load_group("PHASE-25").unwrap();
            black_box(&phase);
        })
    });

    group.finish();
}

fn bench_active_phase_cache(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let storage = Storage::new(Some(temp_dir.path().to_path_buf()));
    storage.initialize().unwrap();

    let mut tasks = HashMap::new();
    tasks.insert("TEST-1".to_string(), Phase::new("TEST-1".to_string()));
    storage.save_tasks(&tasks).unwrap();
    storage.set_active_group("TEST-1").unwrap();

    let mut group = c.benchmark_group("active_phase_cache");

    group.bench_function("first_call_no_cache", |b| {
        b.iter(|| {
            storage.clear_cache();
            let active = storage.get_active_group().unwrap();
            black_box(active);
        })
    });

    group.bench_function("second_call_with_cache", |b| {
        // Prime the cache
        storage.get_active_group().unwrap();

        b.iter(|| {
            let active = storage.get_active_group().unwrap();
            black_box(active);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_load_all_vs_load_one,
    bench_active_phase_cache
);
criterion_main!(benches);
