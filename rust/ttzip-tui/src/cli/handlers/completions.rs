// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Shell completion generator for Bash, Zsh, and Fish.

/// Executes `completions` subcommand.
pub fn execute_completions(shell: &str) -> Result<(), String> {
    match shell.to_lowercase().as_str() {
        "bash" => {
            println!("{}", generate_bash_completions());
            Ok(())
        }
        "zsh" => {
            println!("{}", generate_zsh_completions());
            Ok(())
        }
        "fish" => {
            println!("{}", generate_fish_completions());
            Ok(())
        }
        other => Err(format!(
            "Unsupported shell '{}'. Supported shells: bash, zsh, fish",
            other
        )),
    }
}

fn generate_bash_completions() -> &'static str {
    r#"# Bash completion script for ttzip
_ttzip() {
    local cur prev words cword
    _init_completion || return

    local commands="list extract create recover repair split join bench cat check comment convert delete diff hash info lock tree update doctor completions"

    if [[ $cword -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "${commands} --help --version" -- "$cur") )
        return 0
    fi

    case "${words[1]}" in
        list|l)
            COMPREPLY=( $(compgen -W "-p --password --json -i --include -x --exclude" -- "$cur") )
            ;;
        extract|x)
            COMPREPLY=( $(compgen -W "-o --output -p --password -t --threads -v --verbose -n --dry-run -i --include -x --exclude" -- "$cur") )
            ;;
        create|c)
            COMPREPLY=( $(compgen -W "-f --format -l --level -p --password -t --threads -v --volume-size -n --dry-run -i --include -x --exclude" -- "$cur") )
            ;;
        recover|rec)
            COMPREPLY=( $(compgen -W "-d --dict -t --threads --json" -- "$cur") )
            ;;
        repair|rep)
            COMPREPLY=( $(compgen -W "-o --output -f --format --json" -- "$cur") )
            ;;
        split|sp)
            COMPREPLY=( $(compgen -W "-v --volume-size -o --output-dir -n --naming" -- "$cur") )
            ;;
        join|j)
            COMPREPLY=( $(compgen -W "-o --output --json" -- "$cur") )
            ;;
        bench|b)
            COMPREPLY=( $(compgen -W "--mips --pareto -t --threads -d --dict -i --iterations" -- "$cur") )
            ;;
        cat|view)
            COMPREPLY=( $(compgen -W "-p --password" -- "$cur") )
            ;;
        check|test)
            COMPREPLY=( $(compgen -W "-p --password --deep --json" -- "$cur") )
            ;;
        comment)
            COMPREPLY=( $(compgen -W "-c --comment --json" -- "$cur") )
            ;;
        convert)
            COMPREPLY=( $(compgen -W "-f --format -l --level" -- "$cur") )
            ;;
        delete|d|remove|rm)
            COMPREPLY=( $(compgen -W "--json" -- "$cur") )
            ;;
        diff)
            COMPREPLY=( $(compgen -W "--json" -- "$cur") )
            ;;
        hash|checksum)
            COMPREPLY=( $(compgen -W "-a --algorithm --json" -- "$cur") )
            ;;
        info|inspect|i)
            COMPREPLY=( $(compgen -W "--json" -- "$cur") )
            ;;
        lock)
            COMPREPLY=( $(compgen -W "-u --unlock --json" -- "$cur") )
            ;;
        tree)
            COMPREPLY=( $(compgen -W "-d --depth -i --include -x --exclude --json" -- "$cur") )
            ;;
        update|u)
            COMPREPLY=( $(compgen -W "-l --level --json" -- "$cur") )
            ;;
        doctor|diag)
            COMPREPLY=( $(compgen -W "--json" -- "$cur") )
            ;;
        completions)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
            ;;
        *)
            ;;
    esac
}

complete -F _ttzip ttzip
"#
}

fn generate_zsh_completions() -> &'static str {
    r#"#compdef ttzip

_ttzip() {
    local -a commands
    commands=(
        'list:List entries and metadata inside an archive'
        'extract:Extract archive entries to destination directory'
        'create:Create a new archive from source files/directories'
        'recover:Recover password of encrypted archive using multi-core dictionary attack'
        'repair:Repair damaged ZIP or TAR archive and recover salvageable files'
        'split:Split an archive into multi-volume segments'
        'join:Join multi-volume archive segments into a single file'
        'bench:Run compression benchmark & Pareto frontier visualization'
        'cat:View or dump entry content directly to standard output'
        'check:Check integrity and container format compliance of an archive'
        'comment:Inspect and modify archive comments'
        'convert:Convert an archive into another format with recompression'
        'delete:Delete files/directories from inside an existing archive'
        'diff:Compare structure and content differences between two archives'
        'hash:Calculate streaming cryptographic checksums of an archive or file'
        'info:Inspect detailed archive headers, compression ratio, and metadata'
        'lock:Lock or unlock an archive to prevent accidental modifications'
        'tree:Render visual hierarchical ASCII/Unicode directory tree'
        'update:Incrementally update modified files inside an archive'
        'doctor:Diagnose host environment, CPU SIMD extensions, and format engines'
        'completions:Generate shell completion scripts'
    )

    _arguments -C \
        '1: :->command' \
        '*:: :->args'

    case $state in
        command)
            _describe -t commands 'ttzip command' commands
            ;;
        args)
            case $words[1] in
                list|l)
                    _arguments \
                        '(-p --password)'{-p,--password}'[Optional password]:password: ' \
                        '--json[Output in JSON format]' \
                        '(-i --include)'{-i,--include}'[Include glob patterns]:pattern: ' \
                        '(-x --exclude)'{-x,--exclude}'[Exclude glob patterns]:pattern: ' \
                        '1:archive:_files'
                    ;;
                extract|x)
                    _arguments \
                        '(-o --output)'{-o,--output}'[Destination output directory]:dir:_files -/' \
                        '(-p --password)'{-p,--password}'[Optional password]:password: ' \
                        '(-t --threads)'{-t,--threads}'[Number of parallel threads]:threads: ' \
                        '(-v --verbose)'{-v,--verbose}'[Verbose output]' \
                        '(-n --dry-run)'{-n,--dry-run}'[Dry run simulation mode]' \
                        '(-i --include)'{-i,--include}'[Include glob patterns]:pattern: ' \
                        '(-x --exclude)'{-x,--exclude}'[Exclude glob patterns]:pattern: ' \
                        '1:archive:_files'
                    ;;
                create|c)
                    _arguments \
                        '(-f --format)'{-f,--format}'[Archive format]:format:(zip 7z tar)' \
                        '(-l --level)'{-l,--level}'[Compression level (0-12)]:level: ' \
                        '(-p --password)'{-p,--password}'[Optional password]:password: ' \
                        '(-t --threads)'{-t,--threads}'[Parallel threads]:threads: ' \
                        '(-v --volume-size)'{-v,--volume-size}'[Volume size]:size: ' \
                        '(-n --dry-run)'{-n,--dry-run}'[Dry run simulation mode]' \
                        '(-i --include)'{-i,--include}'[Include glob patterns]:pattern: ' \
                        '(-x --exclude)'{-x,--exclude}'[Exclude glob patterns]:pattern: ' \
                        '1:archive:_files' \
                        '*:sources:_files'
                    ;;
                completions)
                    _arguments '1:shell:(bash zsh fish)'
                    ;;
                *)
                    _files
                    ;;
            esac
            ;;
    esac
}

_ttzip "$@"
"#
}

fn generate_fish_completions() -> &'static str {
    r#"# Fish completion script for ttzip
complete -c ttzip -f

# Subcommands
complete -c ttzip -n "__fish_use_subcommand" -a list -d "List entries and metadata inside an archive"
complete -c ttzip -n "__fish_use_subcommand" -a extract -d "Extract archive entries to destination directory"
complete -c ttzip -n "__fish_use_subcommand" -a create -d "Create a new archive from source files"
complete -c ttzip -n "__fish_use_subcommand" -a recover -d "Recover password of encrypted archive"
complete -c ttzip -n "__fish_use_subcommand" -a repair -d "Repair damaged ZIP or TAR archive"
complete -c ttzip -n "__fish_use_subcommand" -a split -d "Split an archive into multi-volume segments"
complete -c ttzip -n "__fish_use_subcommand" -a join -d "Join multi-volume archive segments"
complete -c ttzip -n "__fish_use_subcommand" -a bench -d "Run compression benchmark & Pareto visualization"
complete -c ttzip -n "__fish_use_subcommand" -a cat -d "View or dump entry content directly"
complete -c ttzip -n "__fish_use_subcommand" -a check -d "Check integrity and container compliance"
complete -c ttzip -n "__fish_use_subcommand" -a comment -d "Inspect and modify archive comments"
complete -c ttzip -n "__fish_use_subcommand" -a convert -d "Convert an archive into another format"
complete -c ttzip -n "__fish_use_subcommand" -a delete -d "Delete files/directories from archive"
complete -c ttzip -n "__fish_use_subcommand" -a diff -d "Compare differences between two archives"
complete -c ttzip -n "__fish_use_subcommand" -a hash -d "Calculate streaming checksums"
complete -c ttzip -n "__fish_use_subcommand" -a info -d "Inspect detailed archive headers"
complete -c ttzip -n "__fish_use_subcommand" -a lock -d "Lock or unlock archive write-protection"
complete -c ttzip -n "__fish_use_subcommand" -a tree -d "Render visual directory tree"
complete -c ttzip -n "__fish_use_subcommand" -a update -d "Incrementally update modified files"
complete -c ttzip -n "__fish_use_subcommand" -a doctor -d "Diagnose host environment and engines"
complete -c ttzip -n "__fish_use_subcommand" -a completions -d "Generate shell completion scripts"

# Completions command
complete -c ttzip -n "__fish_seen_subcommand_from completions" -a "bash zsh fish"

# Flags
complete -c ttzip -s n -l dry-run -d "Dry run simulation mode"
complete -c ttzip -s i -l include -d "Include glob pattern"
complete -c ttzip -s x -l exclude -d "Exclude glob pattern"
complete -c ttzip -l json -d "Output in JSON format"
"#
}
