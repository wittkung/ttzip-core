// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Criterion micro-benchmarks for Extraction Collision Protocol (Feature 229).

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion,
};
use std::fs::File;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use ttzip_archive::fs::collision::fs_ops::{FsOps, LibcFsOps};
use ttzip_archive::fs::collision::preflight::preflight_detect;
use ttzip_archive::fs::collision::session::ExtractionSession;
use ttzip_archive::fs::collision::{CollisionAction, ItemKind, ItemMeta, ResolutionSet};

fn generate_items(count: usize) -> Vec<(PathBuf, ItemMeta)> {
    (0..count)
        .map(|i| {
            (
                PathBuf::from(format!("dir/file_{:06}.txt", i)),
                ItemMeta {
                    kind: ItemKind::File,
                    size: 1024,
                    mtime_unix_ns: 1600000000000000000,
                },
            )
        })
        .collect()
}

fn create_mock_files(dir: &Path, items: &[(PathBuf, ItemMeta)]) {
    let _ = std::fs::create_dir_all(dir.join("dir"));
    for (path, meta) in items {
        let full_path = dir.join(path);
        if let Some(p) = full_path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let mut f = File::create(&full_path).unwrap();
        f.write_all(&vec![0u8; meta.size as usize]).unwrap();
    }
}

fn bench_sc_004_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("SC_004_overhead");
    let items = generate_items(100);

    group.bench_function("baseline_direct_write", |b| {
        b.iter_batched(
            || TempDir::new().unwrap(),
            |tmp| {
                create_mock_files(tmp.path(), &items);
                black_box(tmp);
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("extraction_session_overhead", |b| {
        b.iter_batched(
            || TempDir::new().unwrap(),
            |tmp| {
                let fs = LibcFsOps;
                let session = ExtractionSession::new(tmp.path()).stage().unwrap();
                create_mock_files(tmp.path(), &items); // mock extracting to staging
                let resolutions = ResolutionSet {
                    default_action: Some(CollisionAction::Replace),
                    overrides: vec![],
                };
                session.commit(&fs, &resolutions).unwrap();
                black_box(tmp);
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_sc_005_preflight(c: &mut Criterion) {
    let mut group = c.benchmark_group("SC_005_preflight");
    let fs = LibcFsOps;

    for &size in &[10_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("preflight_empty_dest", size), &size, |b, &s| {
            let items = generate_items(s);
            b.iter_batched(
                || {
                    let tmp = TempDir::new().unwrap();
                    let fd = fs.openat_dir_nofollow(libc::AT_FDCWD, tmp.path()).unwrap();
                    (tmp, fd)
                },
                |(_tmp, fd)| {
                    let dest_root = Path::new("");
                    black_box(preflight_detect(&fs, fd.as_raw_fd(), dest_root, &items).unwrap());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_batch_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("Batch_Commit");
    let fs = LibcFsOps;

    for &size in &[1_000, 10_000] {
        for action in &[CollisionAction::KeepBoth, CollisionAction::Replace, CollisionAction::Skip] {
            let action_name = match action {
                CollisionAction::KeepBoth => "KeepBoth",
                CollisionAction::Replace => "Replace",
                CollisionAction::Skip => "Skip",
            };

            group.bench_with_input(
                BenchmarkId::new(format!("commit_{}", action_name), size),
                &size,
                |b, &s| {
                    let items = generate_items(s);
                    b.iter_batched(
                        || {
                            let tmp = TempDir::new().unwrap();
                            // pre-create staging files
                            let session = ExtractionSession::new(tmp.path()).stage().unwrap();
                            // In a real scenario, staged files exist in session's staging area.
                            // We create them in the tmp directory to simulate.
                            create_mock_files(tmp.path(), &items);
                            (tmp, session)
                        },
                        |(_tmp, session)| {
                            let resolutions = ResolutionSet {
                                default_action: match action {
                                    CollisionAction::KeepBoth => Some(CollisionAction::KeepBoth),
                                    CollisionAction::Replace => Some(CollisionAction::Replace),
                                    CollisionAction::Skip => Some(CollisionAction::Skip),
                                },
                                overrides: vec![],
                            };
                            black_box(session.commit(&fs, &resolutions).unwrap());
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_sc_004_overhead,
    bench_sc_005_preflight,
    bench_batch_commit
);
criterion_main!(benches);
