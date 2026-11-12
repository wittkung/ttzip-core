// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! TTZip Standalone Native CLI & Interactive TUI Application Entry Point.

use clap::{CommandFactory, Parser};
use std::io::IsTerminal;
use std::path::PathBuf;
use ttzip_tui::cli::{
    execute_bench, execute_cat, execute_check, execute_comment, execute_completions,
    execute_convert, execute_create_with_params, execute_delete, execute_diff, execute_doctor,
    execute_extract_unified_with_policy, execute_extract_with_policy, execute_hash, execute_info,
    execute_join, execute_list, execute_list_unified, execute_lock, execute_mcp, execute_recover,
    execute_repair, execute_split, execute_tree, execute_update, run_interactive_tui, Cli,
    CliCreateParams, CliExtractParams, Commands,
};

fn uses_unified_extractor_directly(archive: &std::path::Path) -> bool {
    ttzip_tui::cli::handlers::create::prefers_unified_archive_handler(archive)
        || ttzip_engine::archive::split::detect_volume_chain(archive)
            .is_ok_and(|volumes| volumes.len() > 1)
}

fn is_unrecognized_format_extract_error(err: &str) -> bool {
    err.starts_with("Cannot extract unrecognized archive:")
        || err.contains("Unsupported or unrecognized archive format")
}

fn run_extract_command(
    params: CliExtractParams<'_>,
    on_conflict: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive = params.archive_path;
    let output = params.output_dir;
    let password = params.password;
    let dry_run = params.dry_run;
    let include = params.include;
    let exclude = params.exclude;
    let res = if uses_unified_extractor_directly(archive) {
        execute_extract_unified_with_policy(
            archive,
            output,
            password,
            dry_run,
            include,
            exclude,
            Some(on_conflict),
        )
    } else {
        execute_extract_with_policy(params, Some(on_conflict)).or_else(|err| {
            if is_unrecognized_format_extract_error(&err) {
                execute_extract_unified_with_policy(
                    archive,
                    output,
                    password,
                    dry_run,
                    include,
                    exclude,
                    Some(on_conflict),
                )
            } else {
                Err(err)
            }
        })
    };
    res.map_err(|e| e.into())
}

fn run_list_command(
    archive: & std::path::Path,
    password: Option<&str>,
    json: bool,
    include: &[String],
    exclude: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let res = if ttzip_tui::cli::handlers::create::prefers_unified_archive_handler(archive) {
        execute_list_unified(archive, password, json, include, exclude)
    } else {
        execute_list(archive, password, json, include, exclude)
            .or_else(|_| execute_list_unified(archive, password, json, include, exclude))
    };
    res.map_err(|e| e.into())
}

fn dispatch_core_archive_commands(
    cmd: Commands,
) -> Result<Result<(), Box<dyn std::error::Error>>, Box<Commands>> {
    match cmd {
        Commands::List {
            archive,
            password,
            json,
            include,
            exclude,
        } => Ok(run_list_command(
            &archive,
            password.as_deref(),
            json,
            &include,
            &exclude,
        )),
        Commands::Extract {
            archive,
            output,
            password,
            threads,
            verbose,
            dry_run,
            include,
            exclude,
            on_conflict,
        } => Ok(run_extract_command(
            CliExtractParams {
                archive_path: &archive,
                output_dir: output.as_deref(),
                password: password.as_deref(),
                threads,
                verbose,
                dry_run,
                include: &include,
                exclude: &exclude,
            },
            &on_conflict,
        )),
        Commands::Create {
            archive,
            sources,
            format,
            level,
            password,
            threads,
            volume_size,
            dry_run,
            include,
            exclude,
        } => Ok(
            execute_create_with_params(CliCreateParams {
                archive_path: &archive,
                sources: &sources,
                format_opt: format.as_deref(),
                level,
                password: password.as_deref(),
                threads,
                volume_size: volume_size.as_deref(),
                dry_run,
                include: &include,
                exclude: &exclude,
                silent: false,
            })
            .map_err(|e| e.into()),
        ),
        other => Err(Box::new(other)),
    }
}

fn dispatch_inspection_and_mutation_commands(
    cmd: Commands,
) -> Result<Result<(), Box<dyn std::error::Error>>, Box<Commands>> {
    match cmd {
        Commands::Cat {
            archive,
            entry_path,
            password,
        } => Ok(execute_cat(&archive, &entry_path, password.as_deref()).map_err(|e| e.into())),
        Commands::Check {
            archive,
            password,
            deep,
            json,
        } => Ok(execute_check(&archive, password.as_deref(), deep, json).map_err(|e| e.into())),
        Commands::Comment {
            archive,
            comment,
            json,
        } => Ok(execute_comment(&archive, comment.as_deref(), json).map_err(|e| e.into())),
        Commands::Convert {
            source_archive,
            destination_archive,
            format,
            level,
        } => Ok(execute_convert(&source_archive, &destination_archive, format.as_deref(), level)
            .map_err(|e| e.into())),
        Commands::Delete {
            archive,
            entries,
            json,
        } => Ok(execute_delete(&archive, &entries, json).map_err(|e| e.into())),
        Commands::Diff {
            archive_a,
            archive_b,
            json,
        } => Ok(execute_diff(&archive_a, &archive_b, json).map_err(|e| e.into())),
        Commands::Hash {
            path,
            algorithm,
            json,
        } => Ok(execute_hash(&path, &algorithm, json).map_err(|e| e.into())),
        Commands::Info { archive, json } => Ok(execute_info(&archive, json).map_err(|e| e.into())),
        Commands::Lock {
            archive,
            unlock,
            json,
        } => Ok(execute_lock(&archive, unlock, json).map_err(|e| e.into())),
        Commands::Tree {
            archive,
            depth,
            include,
            exclude,
            json,
        } => Ok(execute_tree(&archive, depth, json, &include, &exclude).map_err(|e| e.into())),
        Commands::Update {
            archive,
            sources,
            level,
            json,
        } => Ok(execute_update(&archive, &sources, level, json).map_err(|e| e.into())),
        other => Err(Box::new(other)),
    }
}

fn dispatch_utility_commands(cmd: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Doctor { json } => execute_doctor(json).map_err(|e| e.into()),
        Commands::Completions { shell } => execute_completions(&shell).map_err(|e| e.into()),
        Commands::Mcp => execute_mcp().map_err(|e| e.into()),
        Commands::Recover {
            archive,
            dictionary,
            threads,
            json,
        } => execute_recover(&archive, &dictionary, threads, json).map_err(|e| e.into()),
        Commands::Repair {
            damaged_archive,
            output,
            format,
            json,
        } => execute_repair(&damaged_archive, &output, format.as_deref(), json).map_err(|e| e.into()),
        Commands::Split {
            source_archive,
            volume_size,
            output_dir,
            naming,
        } => execute_split(&source_archive, &volume_size, output_dir.as_deref(), naming.as_deref())
            .map_err(|e| e.into()),
        Commands::Join {
            first_volume,
            output,
            json,
        } => execute_join(&first_volume, &output, json).map_err(|e| e.into()),
        Commands::Bench {
            mips,
            pareto,
            threads,
            dict_mb,
            iterations,
        } => execute_bench(mips, pareto, threads, dict_mb, iterations).map_err(|e| e.into()),
        _ => Ok(()),
    }
}

fn dispatch_default_command(archive: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(archive_path) = archive {
        if !archive_path.exists() {
            eprintln!("[ERROR] Target archive does not exist: {}", archive_path.display());
            std::process::exit(1);
        }
        if std::io::stdout().is_terminal() {
            run_interactive_tui(archive_path)
        } else {
            execute_list(&archive_path, None, false, &[], &[])
                .or_else(|_| execute_list_unified(&archive_path, None, false, &[], &[]))
                .map_err(|e| e.into())
        }
    } else {
        let mut cmd = Cli::command();
        let _ = cmd.print_help();
        println!();
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(cmd) => match dispatch_core_archive_commands(cmd) {
            Ok(res) => res,
            Err(cmd) => match dispatch_inspection_and_mutation_commands(*cmd) {
                Ok(res) => res,
                Err(cmd) => dispatch_utility_commands(*cmd),
            },
        },
        None => dispatch_default_command(cli.archive),
    };

    if let Err(err) = result {
        eprintln!("[ERROR] {}", err);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_create_preserves_empty_directories() {
        let dir = tempdir().expect("tempdir");
        let root_src = dir.path().join("project");
        let empty_nested = root_src.join("empty_sub");
        let standalone_empty = dir.path().join("only_empty");
        fs::create_dir_all(&empty_nested).expect("create empty_nested");
        fs::create_dir_all(&standalone_empty).expect("create standalone_empty");
        fs::write(root_src.join("readme.txt"), b"hello").expect("write readme");

        let archive_only_empty = dir.path().join("only_empty.zip");
        execute_create_with_params(CliCreateParams {
            archive_path: &archive_only_empty,
            sources: std::slice::from_ref(&standalone_empty),
            format_opt: Some("zip"),
            level: 1,
            password: None,
            threads: 1,
            volume_size: None,
            dry_run: false,
            include: &[],
            exclude: &[],
            silent: true,
        })
        .expect("create archive of only empty directory must succeed");

        let out_only = dir.path().join("out_only");
        execute_extract_with_policy(
            CliExtractParams {
                archive_path: &archive_only_empty,
                output_dir: Some(&out_only),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &[],
                exclude: &[],
            },
            None,
        )
        .expect("extract archive of only empty directory");
        assert!(out_only.join("only_empty").is_dir());

        let archive_nested = dir.path().join("project.tar.gz");
        execute_create_with_params(CliCreateParams {
            archive_path: &archive_nested,
            sources: std::slice::from_ref(&root_src),
            format_opt: Some("tar.gz"),
            level: 1,
            password: None,
            threads: 1,
            volume_size: None,
            dry_run: false,
            include: &[],
            exclude: &[],
            silent: true,
        })
        .expect("create tar.gz with nested empty directory");

        let out_nested = dir.path().join("out_nested");
        execute_extract_unified_with_policy(
            &archive_nested,
            Some(&out_nested),
            None,
            false,
            &[],
            &[],
            Some("replace"),
        )
        .expect("extract tar.gz with nested empty directory");
        assert!(out_nested.join("project/empty_sub").is_dir());
        assert_eq!(
            fs::read(out_nested.join("project/readme.txt")).expect("read readme"),
            b"hello"
        );
    }

    #[test]
    fn test_extract_unified_respects_filters_and_conflict_policy() {
        let dir = tempdir().expect("tempdir");
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).expect("create src");
        let keep_file = src_dir.join("keep.txt");
        let skip_file = src_dir.join("drop.log");
        fs::write(&keep_file, b"updated content").expect("write keep");
        fs::write(&skip_file, b"log content").expect("write drop");

        let archive_path = dir.path().join("bundle.tar.gz");
        execute_create_with_params(CliCreateParams {
            archive_path: &archive_path,
            sources: &[keep_file, skip_file],
            format_opt: Some("tar.gz"),
            level: 1,
            password: None,
            threads: 1,
            volume_size: None,
            dry_run: false,
            include: &[],
            exclude: &[],
            silent: true,
        })
        .expect("create bundle.tar.gz");

        let out_dir = dir.path().join("out");
        let include = vec!["*.txt".to_string()];
        run_extract_command(
            CliExtractParams {
                archive_path: &archive_path,
                output_dir: Some(&out_dir),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &include,
                exclude: &[],
            },
            "replace",
        )
        .expect("filtered unified extract");
        assert!(out_dir.join("keep.txt").exists());
        assert!(!out_dir.join("drop.log").exists());

        fs::write(out_dir.join("keep.txt"), b"existing content").expect("write existing");
        let stop_res = run_extract_command(
            CliExtractParams {
                archive_path: &archive_path,
                output_dir: Some(&out_dir),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &include,
                exclude: &[],
            },
            "stop",
        );
        assert!(stop_res.is_err(), "--on-conflict stop must fail on existing destination");
        assert_eq!(
            fs::read(out_dir.join("keep.txt")).expect("read preserved"),
            b"existing content"
        );
    }

    #[test]
    fn test_run_extract_command_does_not_bypass_conflict_policy_on_fallback() {
        let dir = tempdir().expect("tempdir");
        let src_file = dir.path().join("item.txt");
        fs::write(&src_file, b"archive version").expect("write src");

        let zip_path = dir.path().join("test.zip");
        execute_create_with_params(CliCreateParams {
            archive_path: &zip_path,
            sources: &[src_file],
            format_opt: Some("zip"),
            level: 1,
            password: None,
            threads: 1,
            volume_size: None,
            dry_run: false,
            include: &[],
            exclude: &[],
            silent: true,
        })
        .expect("create zip");

        let out_dir = dir.path().join("out_zip");
        fs::create_dir_all(&out_dir).expect("create out_zip");
        fs::write(out_dir.join("item.txt"), b"preexisting local").expect("write preexisting");

        let res = run_extract_command(
            CliExtractParams {
                archive_path: &zip_path,
                output_dir: Some(&out_dir),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &[],
                exclude: &[],
            },
            "stop",
        );
        assert!(res.is_err(), "zip extract with --on-conflict stop must fail");
        assert_eq!(
            fs::read(out_dir.join("item.txt")).expect("read preexisting"),
            b"preexisting local"
        );
    }

    #[test]
    fn test_filtered_extract_supports_entries_over_64mib() {
        use ttzip_engine::archive::tar::writer::TarWriter;
        use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

        let dir = tempdir().expect("tempdir");
        let archive_path = dir.path().join("large.tar.gz");
        let large_size = 65 * 1024 * 1024usize;
        let mut tar = TarWriter::new(Vec::new());
        tar.append_file("big.bin", &vec![b'A'; large_size], 0o644, 1700000000)
            .expect("append big.bin");
        tar.append_file("skip.txt", b"small", 0o644, 1700000000)
            .expect("append skip.txt");
        tar.finish().expect("finish tar");
        let tar_bytes = tar.into_inner();
        let gz_bytes = libdeflate_gzip_compress(&tar_bytes, 1).expect("gzip compress");
        fs::write(&archive_path, gz_bytes).expect("write large.tar.gz");

        let out_dir = dir.path().join("out_large");
        let include = vec!["big.bin".to_string()];
        execute_extract_with_policy(
            CliExtractParams {
                archive_path: &archive_path,
                output_dir: Some(&out_dir),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &include,
                exclude: &[],
            },
            None,
        )
        .expect("filtered extract >64MiB must succeed via streaming");

        assert_eq!(
            fs::metadata(out_dir.join("big.bin"))
                .expect("stat big.bin")
                .len(),
            large_size as u64
        );
        assert!(!out_dir.join("skip.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_filtered_extract_preserves_symlinks_and_hardlinks() {
        use std::os::unix::fs::MetadataExt;
        use ttzip_engine::archive::tar::header::TYPE_HARDLINK;
        use ttzip_engine::archive::tar::writer::{TarCustomEntryParams, TarWriter};
        use ttzip_engine::codecs::deflate::libdeflate_gzip_compress;

        let dir = tempdir().expect("tempdir");
        let archive_path = dir.path().join("links.tar.gz");
        let mut tar = TarWriter::new(Vec::new());
        tar.append_file("base.txt", b"link target content", 0o644, 1700000000)
            .expect("append base");
        tar.append_symlink("sym.txt", "base.txt", 0o777, 1700000000)
            .expect("append symlink");
        tar.append_custom_entry(TarCustomEntryParams {
            rel_path: "hard.txt",
            data: &[],
            mode: 0o644,
            mtime: 1700000000,
            typeflag: TYPE_HARDLINK,
            link_target: Some("base.txt"),
            pax_extra: None,
        })
        .expect("append hardlink");
        tar.append_file("ignored.log", b"ignore me", 0o644, 1700000000)
            .expect("append ignored");
        tar.finish().expect("finish tar");
        let tar_bytes = tar.into_inner();
        let gz_bytes = libdeflate_gzip_compress(&tar_bytes, 1).expect("gzip compress");
        fs::write(&archive_path, gz_bytes).expect("write links.tar.gz");

        let out_dir = dir.path().join("out");
        let include = vec!["*.txt".to_string()];
        execute_extract_with_policy(
            CliExtractParams {
                archive_path: &archive_path,
                output_dir: Some(&out_dir),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &include,
                exclude: &[],
            },
            None,
        )
        .expect("filtered extract tar.gz with links");

        assert!(!out_dir.join("ignored.log").exists());
        let sym_meta = fs::symlink_metadata(out_dir.join("sym.txt")).expect("stat sym.txt");
        assert!(sym_meta.file_type().is_symlink());
        assert_eq!(fs::read_link(out_dir.join("sym.txt")).expect("read_link"), PathBuf::from("base.txt"));

        let base_meta = fs::metadata(out_dir.join("base.txt")).expect("stat base.txt");
        let hard_meta = fs::metadata(out_dir.join("hard.txt")).expect("stat hard.txt");
        assert_eq!(base_meta.ino(), hard_meta.ino());
        assert_eq!(fs::read(out_dir.join("hard.txt")).expect("read hard.txt"), b"link target content");
    }

    #[test]
    fn test_filtered_tar_extract_rejects_symlink_escape_through_jail() {
        use ttzip_engine::archive::tar::writer::TarWriter;

        let dir = tempdir().expect("tempdir");
        let outside_dir = dir.path().join("outside");
        fs::create_dir_all(&outside_dir).expect("create outside_dir");
        let outside_target = outside_dir.to_string_lossy().to_string();

        let archive_path = dir.path().join("escape.tar");
        let mut tar = TarWriter::new(Vec::new());
        tar.append_symlink("link", &outside_target, 0o777, 1700000000)
            .expect("append symlink");
        tar.append_file("link/escaped.txt", b"escaped payload", 0o644, 1700000000)
            .expect("append escaped payload");
        tar.finish().expect("finish tar");
        fs::write(&archive_path, tar.into_inner()).expect("write escape.tar");

        let out_dir = dir.path().join("dest");
        let include = vec!["*".to_string()];
        let res = execute_extract_with_policy(
            CliExtractParams {
                archive_path: &archive_path,
                output_dir: Some(&out_dir),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &include,
                exclude: &[],
            },
            None,
        );
        assert!(res.is_err(), "jailed TAR extraction must reject outside symlink target");
        assert!(!outside_dir.join("escaped.txt").exists());
    }

    #[test]
    fn test_execute_list_and_extract_engine_formats() {
        use ttzip_engine::archive::unified::create_archive;
        use ttzip_engine::types::{TTZipArchiveFormat, TTZipCompressionLevel, TTZipCreateOptions};

        let dir = tempdir().expect("tempdir");
        let src_dir = dir.path().join("src");
        fs::create_dir_all(&src_dir).expect("create src dir");
        let sample_file = src_dir.join("note.txt");
        let payload = b"engine format parity verification payload";
        fs::write(&sample_file, payload).expect("write sample file");

        for (ext, fmt) in [("bundle.tar.gz", TTZipArchiveFormat::TarGz), ("bundle.cpio", TTZipArchiveFormat::Cpio)] {
            let archive_path = dir.path().join(ext);
            let opts = TTZipCreateOptions {
                format: fmt,
                level: TTZipCompressionLevel::Normal,
                thread_budget: 1,
                ..Default::default()
            };
            create_archive(std::slice::from_ref(&sample_file), &archive_path, &opts, 0)
                .expect("create engine archive");

            execute_list(&archive_path, None, true, &[], &[]).expect("list engine archive");

            let out_dir = dir.path().join(format!("out_{}", ext));
            execute_extract_with_policy(
                CliExtractParams {
                    archive_path: &archive_path,
                    output_dir: Some(&out_dir),
                    password: None,
                    threads: 1,
                    verbose: false,
                    dry_run: false,
                    include: &[],
                    exclude: &[],
                },
                None,
            )
            .expect("extract engine archive");
            let extracted_bytes = fs::read(out_dir.join("note.txt")).expect("read extracted file");
            assert_eq!(extracted_bytes, payload);
        }
    }

    #[test]
    fn test_multi_volume_zip_and_sevenz_over_64kib_extract_via_cli() {
        use ttzip_engine::sevenz::writer::create_7z_solid_archive_bytes_with_password;
        use ttzip_engine::types::TTZipEncryptionMethod;
        use ttzip_engine::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

        let dir = tempdir().expect("tempdir");
        let mut payload = vec![0u8; 96 * 1024];
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for byte in &mut payload {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = (state & 0xFF) as u8;
        }

        let items = vec![ZipInputItem {
            rel_path: "split_payload.bin".to_string(),
            data: payload.clone(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed =
            compress_items_parallel(items.clone(), 0, TTZipEncryptionMethod::None, None, 1)
                .expect("compress zip");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assemble zip");
        assert!(zip_bytes.len() > 65536);
        let zip_mid = zip_bytes.len() / 2;
        let zip_vol1 = dir.path().join("split_bundle.zip.001");
        let zip_vol2 = dir.path().join("split_bundle.zip.002");
        fs::write(&zip_vol1, &zip_bytes[..zip_mid]).expect("write zip.001");
        fs::write(&zip_vol2, &zip_bytes[zip_mid..]).expect("write zip.002");

        let zip_out = dir.path().join("out_zip");
        run_extract_command(
            CliExtractParams {
                archive_path: &zip_vol1,
                output_dir: Some(&zip_out),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &[],
                exclude: &[],
            },
            "keep-both",
        )
        .expect("extract split zip >64KiB via run_extract_command");
        assert_eq!(fs::read(zip_out.join("split_payload.bin")).expect("read split zip payload"), payload);

        let sz_bytes =
            create_7z_solid_archive_bytes_with_password(&items, 0, 1, None).expect("assemble 7z");
        assert!(sz_bytes.len() > 65536);
        let sz_mid = sz_bytes.len() / 2;
        let sz_vol1 = dir.path().join("split_bundle.7z.001");
        let sz_vol2 = dir.path().join("split_bundle.7z.002");
        fs::write(&sz_vol1, &sz_bytes[..sz_mid]).expect("write 7z.001");
        fs::write(&sz_vol2, &sz_bytes[sz_mid..]).expect("write 7z.002");

        let sz_out = dir.path().join("out_7z");
        execute_extract_with_policy(
            CliExtractParams {
                archive_path: &sz_vol1,
                output_dir: Some(&sz_out),
                password: None,
                threads: 1,
                verbose: false,
                dry_run: false,
                include: &[],
                exclude: &[],
            },
            Some("keep-both"),
        )
        .expect("extract split 7z >64KiB via execute_extract_with_policy");
        assert_eq!(fs::read(sz_out.join("split_payload.bin")).expect("read split 7z payload"), payload);
    }
}
