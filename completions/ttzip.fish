# fish completion for ttzip

complete -c ttzip -f

# Global options
complete -c ttzip -s h -l help -d "Show help information"
complete -c ttzip -s v -l version -d "Show version information"

# Subcommands
complete -c ttzip -n "__fish_use_subcommand" -a create -d "Compress and pack source files into archive"
complete -c ttzip -n "__fish_use_subcommand" -a extract -d "Extract archive contents to destination"
complete -c ttzip -n "__fish_use_subcommand" -a list -d "List entries in archive"
complete -c ttzip -n "__fish_use_subcommand" -a inspect -d "Deep structural header analysis"
complete -c ttzip -n "__fish_use_subcommand" -a test -d "Verify archive cryptographic integrity"
complete -c ttzip -n "__fish_use_subcommand" -a bench -d "Run hardware SIMD and codec benchmarks"
complete -c ttzip -n "__fish_use_subcommand" -a salvage -d "Recover damaged archive payloads"
complete -c ttzip -n "__fish_use_subcommand" -a doctor -d "Inspect CPU capabilities and SIMD ISA"

# Subcommand options
complete -c ttzip -n "__fish_seen_subcommand_from create" -s l -l level -a "0 1 3 6 9 12" -d "Compression level"
complete -c ttzip -n "__fish_seen_subcommand_from create" -s p -l password -d "Encryption password"
complete -c ttzip -n "__fish_seen_subcommand_from create" -s s -l split -d "Split volume size (e.g. 100M)"
complete -c ttzip -n "__fish_seen_subcommand_from extract" -s o -l output -d "Destination extraction directory"
complete -c ttzip -n "__fish_seen_subcommand_from extract" -s p -l password -d "Decryption password"
complete -c ttzip -n "__fish_seen_subcommand_from extract" -s f -l force -d "Overwrite existing files"
complete -c ttzip -n "__fish_seen_subcommand_from bench" -a "matrix gate deflate zstd snappy" -d "Benchmark target"
