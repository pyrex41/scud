//! Performance benchmarks for extension loading and discovery
//!
//! Run with: cargo bench --bench extension_bench
//!
//! ## Baseline Metrics (measured on Apple M-series, 2026-01-23)
//!
//! ### Manifest Parsing (from string)
//! | Operation                      | Time       | Notes                           |
//! |-------------------------------|------------|----------------------------------|
//! | Minimal manifest              | ~3.5 µs    | 4 required fields only           |
//! | Full manifest                 | ~40 µs     | 2 tools, 3 events, config, deps  |
//! | Legacy agent (minimal)        | ~4.5 µs    | agent section only               |
//! | Legacy agent (full)           | ~12.8 µs   | agent + model + prompt           |
//!
//! ### Manifest Parsing (from file)
//! | Operation                      | Time       | Notes                           |
//! |-------------------------------|------------|----------------------------------|
//! | Minimal from file             | ~21 µs     | Includes filesystem read         |
//! | Full from file                | ~51 µs     | Includes filesystem read         |
//! | Legacy from file              | ~27 µs     | Includes filesystem read         |
//!
//! ### Validation
//! | Operation                      | Time       | Notes                           |
//! |-------------------------------|------------|----------------------------------|
//! | Validate minimal              | ~19 ns     | ID + version + basic checks      |
//! | Validate full                 | ~33 ns     | Tools + events + params          |
//!
//! ### Discovery (walkdir + parsing + validation)
//! | Operation                      | Time       | Notes                           |
//! |-------------------------------|------------|----------------------------------|
//! | Empty directory               | ~21 µs     | Just walkdir overhead            |
//! | 1 extension                   | ~68 µs     | Single extension.toml            |
//! | 10 extensions                 | ~357 µs    | Flat directory structure         |
//! | 50 extensions                 | ~1.7 ms    | Flat directory structure         |
//! | Depth 4 nested tree           | ~1.9 ms    | Exponential directory growth     |
//! | Limited depth (2)             | ~199 µs    | max_depth optimization           |
//!
//! ### Registry Operations
//! | Operation                      | Time       | Notes                           |
//! |-------------------------------|------------|----------------------------------|
//! | discover_and_load (20 ext)    | ~622 µs    | Full pipeline                    |
//! | get_extension                 | ~11 ns     | HashMap lookup                   |
//! | has_extension                 | ~10 ns     | HashMap contains_key             |
//! | list_ids                      | ~41 ns     | Clone keys to Vec                |
//! | list_all                      | ~35 ns     | Collect values to Vec            |
//! | filter_legacy                 | ~16 ns     | Iterate + filter                 |
//!
//! ### Scaling (registry load)
//! | Extensions | Time       | Per-extension |
//! |-----------|------------|---------------|
//! | 10        | ~317 µs    | ~32 µs        |
//! | 25        | ~776 µs    | ~31 µs        |
//! | 50        | ~1.56 ms   | ~31 µs        |
//! | 100       | ~3.34 ms   | ~33 µs        |
//!
//! ## Optimization Opportunities
//!
//! 1. **Parallel discovery**: walkdir is single-threaded; rayon could parallelize
//! 2. **Lazy parsing**: Parse manifests on-demand rather than during discovery
//! 3. **Caching**: Cache validated manifests to skip re-parsing unchanged files
//! 4. **Memory mapping**: Use mmap for large manifest files
//! 5. **SIMD TOML**: Consider simd-json style optimizations for TOML parsing

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use scud::extensions::{
    discover, discover_all, DiscoveryOptions, ExtensionManifest, ExtensionRegistry,
};
use std::path::PathBuf;
use tempfile::TempDir;

// =============================================================================
// Test Data
// =============================================================================

const MINIMAL_MANIFEST: &str = r#"
[extension]
id = "minimal-ext"
name = "Minimal Extension"
version = "1.0.0"
description = "A minimal test extension"
"#;

const FULL_MANIFEST: &str = r#"
[extension]
id = "full-extension"
name = "Full Extension"
version = "2.0.0"
description = "A fully featured extension for benchmarking"
author = "Benchmark Suite"
main = "main.py"
license = "MIT"
homepage = "https://example.com"
repository = "https://github.com/example/extension"

[[tools]]
name = "analyze"
description = "Analyze a file"
command = "python analyze.py ${file}"

[[tools.parameters]]
name = "file"
type = "string"
description = "Path to file"
required = true

[[tools.parameters]]
name = "verbose"
type = "boolean"
required = false
default = false

[[tools]]
name = "transform"
description = "Transform data"
script = "scripts/transform.py"

[[tools.parameters]]
name = "input"
type = "string"
required = true

[[tools.parameters]]
name = "format"
type = "string"
required = false
default = "json"

[[events]]
event = "task.start"
type = "command"
command = "echo 'Starting'"

[[events]]
event = "task.complete"
type = "script"
script = "on_complete.sh"

[[events]]
event = "session.end"
type = "webhook"
url = "https://hooks.example.com/notify"

[config]
api_key = "test-key"
max_retries = 3
debug = true
features = ["feature1", "feature2", "feature3"]

[dependencies]
"core-utils" = ">=1.0.0"
"helper-lib" = "^2.0"
"#;

const LEGACY_AGENT_MINIMAL: &str = r#"
[agent]
name = "test-agent"
description = "A test agent"
"#;

const LEGACY_AGENT_FULL: &str = r#"
[agent]
name = "full-agent"
description = "A fully configured test agent for benchmarking purposes"

[model]
harness = "claude"
model = "opus"

[prompt]
template = """
You are a helpful assistant that performs code analysis.
Given a task, you will:
1. Analyze the code structure
2. Identify potential issues
3. Suggest improvements
4. Implement changes if requested

Always follow best practices and maintain code quality.
"""
"#;

// =============================================================================
// Manifest Parsing Benchmarks
// =============================================================================

fn bench_parse_manifest_from_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("manifest_parsing");

    group.bench_function("minimal_manifest", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_str(
                black_box(MINIMAL_MANIFEST),
                &PathBuf::from("test.toml"),
            )
            .unwrap();
            black_box(manifest)
        })
    });

    group.bench_function("full_manifest", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_str(
                black_box(FULL_MANIFEST),
                &PathBuf::from("test.toml"),
            )
            .unwrap();
            black_box(manifest)
        })
    });

    group.bench_function("legacy_agent_minimal", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_str(
                black_box(LEGACY_AGENT_MINIMAL),
                &PathBuf::from("agent.toml"),
            )
            .unwrap();
            black_box(manifest)
        })
    });

    group.bench_function("legacy_agent_full", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_str(
                black_box(LEGACY_AGENT_FULL),
                &PathBuf::from("agent.toml"),
            )
            .unwrap();
            black_box(manifest)
        })
    });

    group.finish();
}

fn bench_parse_manifest_from_file(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();

    // Write test files
    let minimal_path = temp_dir.path().join("minimal.toml");
    let full_path = temp_dir.path().join("full.toml");
    let legacy_path = temp_dir.path().join("legacy.toml");

    std::fs::write(&minimal_path, MINIMAL_MANIFEST).unwrap();
    std::fs::write(&full_path, FULL_MANIFEST).unwrap();
    std::fs::write(&legacy_path, LEGACY_AGENT_FULL).unwrap();

    let mut group = c.benchmark_group("manifest_file_loading");

    group.bench_function("minimal_from_file", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_file(black_box(&minimal_path)).unwrap();
            black_box(manifest)
        })
    });

    group.bench_function("full_from_file", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_file(black_box(&full_path)).unwrap();
            black_box(manifest)
        })
    });

    group.bench_function("legacy_from_file", |b| {
        b.iter(|| {
            let manifest = ExtensionManifest::from_file(black_box(&legacy_path)).unwrap();
            black_box(manifest)
        })
    });

    group.finish();
}

// =============================================================================
// Validation Benchmarks
// =============================================================================

fn bench_validate_manifest(c: &mut Criterion) {
    let minimal =
        ExtensionManifest::from_str(MINIMAL_MANIFEST, &PathBuf::from("test.toml")).unwrap();
    let full = ExtensionManifest::from_str(FULL_MANIFEST, &PathBuf::from("test.toml")).unwrap();

    let mut group = c.benchmark_group("manifest_validation");

    group.bench_function("validate_minimal", |b| {
        b.iter(|| {
            let result = black_box(&minimal).validate();
            black_box(result)
        })
    });

    group.bench_function("validate_full", |b| {
        b.iter(|| {
            let result = black_box(&full).validate();
            black_box(result)
        })
    });

    group.finish();
}

// =============================================================================
// Discovery Benchmarks
// =============================================================================

/// Create a test directory structure with N extensions
fn create_extension_tree(dir: &TempDir, count: usize) {
    for i in 0..count {
        let ext_dir = dir.path().join(format!("ext-{}", i));
        std::fs::create_dir_all(&ext_dir).unwrap();

        let manifest = format!(
            r#"
[extension]
id = "ext-{}"
name = "Extension {}"
version = "1.0.0"
description = "Test extension number {}"

[[tools]]
name = "tool_{}"
description = "A tool"
command = "echo test"
"#,
            i, i, i, i
        );

        std::fs::write(ext_dir.join("extension.toml"), manifest).unwrap();
    }
}

/// Create a directory tree with legacy agents
fn create_legacy_agent_tree(dir: &TempDir, count: usize) {
    let agents_dir = dir.path().join("spawn-agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    for i in 0..count {
        let manifest = format!(
            r#"
[agent]
name = "agent-{}"
description = "Test agent number {}"

[model]
harness = "claude"
model = "opus"
"#,
            i, i
        );

        std::fs::write(agents_dir.join(format!("agent-{}.toml", i)), manifest).unwrap();
    }
}

/// Create a deeply nested directory structure
fn create_deep_tree(dir: &TempDir, depth: usize, extensions_per_level: usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    // Reset counter for each test
    COUNTER.store(0, Ordering::SeqCst);

    fn create_level(path: &std::path::Path, current_depth: usize, max_depth: usize, ext_count: usize, counter: &AtomicUsize) {
        if current_depth > max_depth {
            return;
        }

        // Create extensions at this level
        for _ in 0..ext_count {
            let unique_id = counter.fetch_add(1, Ordering::SeqCst);
            let ext_dir = path.join(format!("ext-{}", unique_id));
            std::fs::create_dir_all(&ext_dir).unwrap();

            let manifest = format!(
                r#"
[extension]
id = "ext-deep-{}"
name = "Extension at depth {}"
version = "1.0.0"
description = "Test extension at depth {}"
"#,
                unique_id, current_depth, current_depth
            );

            std::fs::write(ext_dir.join("extension.toml"), manifest).unwrap();
        }

        // Create subdirectories
        for i in 0..2 {
            let subdir = path.join(format!("subdir-{}", i));
            std::fs::create_dir_all(&subdir).unwrap();
            create_level(&subdir, current_depth + 1, max_depth, ext_count, counter);
        }
    }

    create_level(dir.path(), 0, depth, extensions_per_level, &COUNTER);
}

fn bench_discover_extensions(c: &mut Criterion) {
    let mut group = c.benchmark_group("extension_discovery");

    // Benchmark empty directory
    group.bench_function("empty_directory", |b| {
        let temp_dir = TempDir::new().unwrap();
        b.iter(|| {
            let result = discover(black_box(temp_dir.path()), DiscoveryOptions::standard()).unwrap();
            black_box(result)
        })
    });

    // Benchmark with varying extension counts
    for count in [1, 5, 10, 25, 50] {
        group.bench_with_input(
            BenchmarkId::new("flat_extensions", count),
            &count,
            |b, &count| {
                let temp_dir = TempDir::new().unwrap();
                create_extension_tree(&temp_dir, count);
                b.iter(|| {
                    let result =
                        discover(black_box(temp_dir.path()), DiscoveryOptions::standard()).unwrap();
                    black_box(result)
                })
            },
        );
    }

    // Benchmark legacy agent discovery
    for count in [1, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("legacy_agents", count),
            &count,
            |b, &count| {
                let temp_dir = TempDir::new().unwrap();
                create_legacy_agent_tree(&temp_dir, count);
                b.iter(|| {
                    let result =
                        discover(black_box(temp_dir.path()), DiscoveryOptions::standard()).unwrap();
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

fn bench_discover_deep_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_discovery");

    // Benchmark discovery at different depths
    for depth in [2, 3, 4] {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &depth| {
            let temp_dir = TempDir::new().unwrap();
            create_deep_tree(&temp_dir, depth, 1);
            b.iter(|| {
                let result =
                    discover(black_box(temp_dir.path()), DiscoveryOptions::standard()).unwrap();
                black_box(result)
            })
        });
    }

    // Benchmark with max_depth limit
    group.bench_function("limited_depth_scan", |b| {
        let temp_dir = TempDir::new().unwrap();
        create_deep_tree(&temp_dir, 5, 2);

        let options = DiscoveryOptions {
            max_depth: Some(2),
            ..DiscoveryOptions::standard()
        };

        b.iter(|| {
            let result = discover(black_box(temp_dir.path()), options.clone()).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

fn bench_discover_options(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    create_extension_tree(&temp_dir, 10);
    create_legacy_agent_tree(&temp_dir, 5);

    let mut group = c.benchmark_group("discovery_options");

    group.bench_function("standard_options", |b| {
        b.iter(|| {
            let result =
                discover(black_box(temp_dir.path()), DiscoveryOptions::standard()).unwrap();
            black_box(result)
        })
    });

    group.bench_function("lenient_options", |b| {
        b.iter(|| {
            let result =
                discover(black_box(temp_dir.path()), DiscoveryOptions::lenient()).unwrap();
            black_box(result)
        })
    });

    group.bench_function("no_validation", |b| {
        let options = DiscoveryOptions {
            validate: false,
            ..DiscoveryOptions::standard()
        };
        b.iter(|| {
            let result = discover(black_box(temp_dir.path()), options.clone()).unwrap();
            black_box(result)
        })
    });

    group.bench_function("no_legacy", |b| {
        let options = DiscoveryOptions {
            include_legacy: false,
            ..DiscoveryOptions::standard()
        };
        b.iter(|| {
            let result = discover(black_box(temp_dir.path()), options.clone()).unwrap();
            black_box(result)
        })
    });

    group.finish();
}

/// Create a test directory structure with N extensions using a unique prefix
fn create_extension_tree_with_prefix(dir: &TempDir, count: usize, prefix: &str) {
    for i in 0..count {
        let ext_dir = dir.path().join(format!("ext-{}-{}", prefix, i));
        std::fs::create_dir_all(&ext_dir).unwrap();

        let manifest = format!(
            r#"
[extension]
id = "ext-{}-{}"
name = "Extension {} {}"
version = "1.0.0"
description = "Test extension number {}"

[[tools]]
name = "tool_{}"
description = "A tool"
command = "echo test"
"#,
            prefix, i, prefix, i, i, i
        );

        std::fs::write(ext_dir.join("extension.toml"), manifest).unwrap();
    }
}

/// Create a directory tree with legacy agents using a unique prefix
fn create_legacy_agent_tree_with_prefix(dir: &TempDir, count: usize, prefix: &str) {
    let agents_dir = dir.path().join("spawn-agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    for i in 0..count {
        let manifest = format!(
            r#"
[agent]
name = "agent-{}-{}"
description = "Test agent number {}"

[model]
harness = "claude"
model = "opus"
"#,
            prefix, i, i
        );

        std::fs::write(agents_dir.join(format!("agent-{}-{}.toml", prefix, i)), manifest).unwrap();
    }
}

fn bench_discover_all_multiple_roots(c: &mut Criterion) {
    let mut group = c.benchmark_group("discover_all");

    for root_count in [2, 3, 5] {
        group.bench_with_input(
            BenchmarkId::new("roots", root_count),
            &root_count,
            |b, &root_count| {
                let temp_dirs: Vec<TempDir> = (0..root_count).map(|_| TempDir::new().unwrap()).collect();

                for (i, dir) in temp_dirs.iter().enumerate() {
                    let prefix = format!("root{}", i);
                    create_extension_tree_with_prefix(dir, 5, &prefix);
                    if i % 2 == 0 {
                        create_legacy_agent_tree_with_prefix(dir, 3, &prefix);
                    }
                }

                let roots: Vec<&std::path::Path> = temp_dirs.iter().map(|d| d.path()).collect();

                b.iter(|| {
                    let result =
                        discover_all(black_box(&roots), DiscoveryOptions::standard()).unwrap();
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Registry Benchmarks
// =============================================================================

fn bench_registry_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    create_extension_tree(&temp_dir, 20);

    let mut group = c.benchmark_group("registry_operations");

    group.bench_function("discover_and_load", |b| {
        b.iter(|| {
            let mut registry = ExtensionRegistry::new();
            registry
                .discover_and_load(black_box(temp_dir.path()), DiscoveryOptions::standard())
                .unwrap();
            black_box(registry)
        })
    });

    // Pre-load registry for lookup benchmarks
    let mut loaded_registry = ExtensionRegistry::new();
    loaded_registry
        .discover_and_load(temp_dir.path(), DiscoveryOptions::standard())
        .unwrap();

    group.bench_function("get_extension", |b| {
        b.iter(|| {
            let ext = loaded_registry.get(black_box("ext-10"));
            black_box(ext)
        })
    });

    group.bench_function("has_extension", |b| {
        b.iter(|| {
            let has = loaded_registry.has(black_box("ext-10"));
            black_box(has)
        })
    });

    group.bench_function("list_ids", |b| {
        b.iter(|| {
            let ids = loaded_registry.list_ids();
            black_box(ids)
        })
    });

    group.bench_function("list_all", |b| {
        b.iter(|| {
            let all = loaded_registry.list();
            black_box(all)
        })
    });

    group.bench_function("filter_legacy", |b| {
        b.iter(|| {
            let legacy = loaded_registry.legacy_extensions();
            black_box(legacy)
        })
    });

    group.finish();
}

fn bench_registry_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_scaling");

    for count in [10, 25, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_extensions", count),
            &count,
            |b, &count| {
                let temp_dir = TempDir::new().unwrap();
                create_extension_tree(&temp_dir, count);

                b.iter(|| {
                    let mut registry = ExtensionRegistry::new();
                    registry
                        .discover_and_load(black_box(temp_dir.path()), DiscoveryOptions::standard())
                        .unwrap();
                    black_box(registry)
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Criterion Setup
// =============================================================================

criterion_group!(
    benches,
    bench_parse_manifest_from_str,
    bench_parse_manifest_from_file,
    bench_validate_manifest,
    bench_discover_extensions,
    bench_discover_deep_tree,
    bench_discover_options,
    bench_discover_all_multiple_roots,
    bench_registry_operations,
    bench_registry_scaling,
);

criterion_main!(benches);
