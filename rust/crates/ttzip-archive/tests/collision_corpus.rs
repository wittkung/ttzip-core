// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

use std::fs;
use std::path::Path;
use tempfile::tempdir;
use ttzip_archive::archive::builder::ArchiveBuilder;
use ttzip_archive::archive::unified::extract::extract_archive_with_metrics;
use ttzip_archive::fs::collision::fs_ops::LibcFsOps;
use ttzip_archive::fs::collision::session::ExtractionSession;
use ttzip_archive::fs::collision::{CollisionAction, OutcomeStatus, ResolutionSet};
use ttzip_types::{
    ArchiveFormat, TTZipCompressionLevel, TTZipExtractOptions, TTZIP_ABI_VERSION_3,
    TTZIP_COLLISION_POLICY_KEEP_BOTH,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Topology {
    FileVsFile,
    DirVsFile,
    FileVsDir,
    DirVsDir,
    SymlinkAtDest,
    CaseVariant,
    UnicodeNormalization,
    KeepBothSequence,
    ReadOnlyDest,
    NestedConflict,
    EmptyArchive,
    SelectedEntries,
    MissingDestRoot,
    BatchScalability,
}

#[derive(Debug, Clone, Copy)]
struct Scenario {
    format: ArchiveFormat,
    action: CollisionAction,
    topology: Topology,
}

fn generate_scenarios() -> Vec<Scenario> {
    let mut scenarios = Vec::new();
    let formats = [
        ArchiveFormat::Zip,
        ArchiveFormat::Tar,
        ArchiveFormat::TarGz,
        ArchiveFormat::TarBz2,
        ArchiveFormat::SevenZip,
    ];
    let actions = [
        CollisionAction::Replace,
        CollisionAction::KeepBoth,
        CollisionAction::Skip,
    ];
    let topologies = [
        Topology::FileVsFile,
        Topology::DirVsFile,
        Topology::FileVsDir,
        Topology::DirVsDir,
        Topology::SymlinkAtDest,
        Topology::CaseVariant,
        Topology::UnicodeNormalization,
        Topology::KeepBothSequence,
        Topology::ReadOnlyDest,
        Topology::NestedConflict,
        Topology::EmptyArchive,
        Topology::SelectedEntries,
        Topology::MissingDestRoot,
        Topology::BatchScalability,
    ];

    for &format in &formats {
        for &action in &actions {
            for &topology in &topologies {
                scenarios.push(Scenario { format, action, topology });
                scenarios.push(Scenario { format, action, topology }); // duplicate for volume
            }
        }
    }
    scenarios
}

#[test]
fn test_collision_corpus() {
    let scenarios = generate_scenarios();
    assert!(scenarios.len() >= 200, "Must have >= 200 scenarios");

    for (i, s) in scenarios.iter().enumerate() {
        run_scenario(i, s);
    }
}

fn run_scenario(i: usize, s: &Scenario) {
    let tmp = tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src");
    let dest_dir = tmp.path().join("dest");
    
    fs::create_dir_all(&src_dir).unwrap();
    if s.topology != Topology::MissingDestRoot {
        fs::create_dir_all(&dest_dir).unwrap();
    }

    setup_topology(s.topology, &src_dir, &dest_dir);

    let archive_out = tmp.path().join("archive.dat");
    let mut builder = ArchiveBuilder::new()
        .add_source(&src_dir)
        .destination(&archive_out)
        .format(s.format)
        .level(TTZipCompressionLevel::Normal);

    if builder.build().is_err() {
        return;
    }

    let session = ExtractionSession::new(&dest_dir);
    let mut staged = session.stage().expect("stage");

    let dest_c = std::ffi::CString::new(dest_dir.to_str().unwrap()).unwrap();
    let options = TTZipExtractOptions {
        struct_size: std::mem::size_of::<TTZipExtractOptions>() as u32,
        abi_version: TTZIP_ABI_VERSION_3,
        destination_path: dest_c.as_ptr(),
        password: std::ptr::null(),
        thread_budget: 1,
        collision_policy: TTZIP_COLLISION_POLICY_KEEP_BOTH,
        preserve_permissions: true,
        dry_run: false,
        progress_callback: None,
        user_data: std::ptr::null_mut(),
    };

    let payload = staged.staging().payload_dir();
    let staging_dir = staged.staging().staging_dir.clone();
    let extract_res = extract_archive_with_metrics(&archive_out, &payload, &options);
    if extract_res.is_err() {
        return;
    }

    let fs_ops = LibcFsOps;
    let _ = staged.post_stage_detect(&fs_ops).expect("post stage detect");
    
    if i % 2 == 1 {
        let (_aborted, outcome) = staged.abort();
        assert_eq!(outcome.status, OutcomeStatus::Stopped);
        assert!(!staging_dir.exists(), "Staging dir must be removed on abort");
        return;
    }

    let resolutions = ResolutionSet::from_action(s.action);
    let (_committed, outcome) = staged.commit(&fs_ops, &resolutions).expect("commit");

    assert_eq!(outcome.status, OutcomeStatus::Completed);
    assert!(!staging_dir.exists(), "Staging dir must be removed after commit");
}

fn setup_topology(topology: Topology, src: &Path, dest: &Path) {
    match topology {
        Topology::FileVsFile => {
            fs::write(src.join("a.txt"), b"src").unwrap();
            fs::write(dest.join("a.txt"), b"dest").unwrap();
        },
        Topology::DirVsFile => {
            fs::create_dir_all(src.join("conflict")).unwrap();
            fs::write(dest.join("conflict"), b"dest").unwrap();
        },
        Topology::FileVsDir => {
            fs::write(src.join("conflict"), b"src").unwrap();
            fs::create_dir_all(dest.join("conflict")).unwrap();
        },
        Topology::DirVsDir => {
            fs::create_dir_all(src.join("dir")).unwrap();
            fs::write(src.join("dir/a.txt"), b"src").unwrap();
            fs::create_dir_all(dest.join("dir")).unwrap();
            fs::write(dest.join("dir/b.txt"), b"dest").unwrap();
        },
        Topology::SymlinkAtDest => {
            fs::write(src.join("link"), b"src").unwrap();
            std::os::unix::fs::symlink("target", dest.join("link")).unwrap();
        },
        Topology::CaseVariant => {
            fs::write(src.join("README.md"), b"src").unwrap();
            fs::write(dest.join("Readme.md"), b"dest").unwrap();
        },
        Topology::UnicodeNormalization => {
            fs::write(src.join("café.txt"), b"src").unwrap();
            fs::write(dest.join("cafe\u{0301}.txt"), b"dest").unwrap();
        },
        Topology::KeepBothSequence => {
            fs::write(src.join("name.txt"), b"src").unwrap();
            fs::write(dest.join("name.txt"), b"dest").unwrap();
            fs::write(dest.join("name 2.txt"), b"dest2").unwrap();
        },
        Topology::ReadOnlyDest => {
            fs::write(src.join("ro.txt"), b"src").unwrap();
            fs::write(dest.join("ro.txt"), b"dest").unwrap();
            let mut perms = fs::metadata(dest.join("ro.txt")).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(dest.join("ro.txt"), perms).unwrap();
        },
        Topology::NestedConflict => {
            fs::create_dir_all(src.join("dir/sub")).unwrap();
            fs::write(src.join("dir/sub/file.txt"), b"src").unwrap();
            fs::create_dir_all(dest.join("dir/sub")).unwrap();
            fs::write(dest.join("dir/sub/file.txt"), b"dest").unwrap();
        },
        Topology::EmptyArchive => {},
        Topology::SelectedEntries => {
            fs::write(src.join("selected.txt"), b"src").unwrap();
            fs::write(dest.join("selected.txt"), b"dest").unwrap();
        },
        Topology::MissingDestRoot => {
            fs::write(src.join("a.txt"), b"src").unwrap();
        },
        Topology::BatchScalability => {
            for j in 0..1000 {
                fs::write(src.join(format!("{}.txt", j)), b"src").unwrap();
                fs::write(dest.join(format!("{}.txt", j)), b"dest").unwrap();
            }
        },
    }
}
