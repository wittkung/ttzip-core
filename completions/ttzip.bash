# bash completion for ttzip

_ttzip() {
    local cur prev words cword
    _init_completion || return

    local subcommands="create extract list inspect test bench salvage doctor help"

    if [[ $cword -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "${subcommands}" -- "$cur") )
        return 0
    fi

    case "${words[1]}" in
        create)
            case "$prev" in
                -l|--level)
                    COMPREPLY=( $(compgen -W "0 1 3 6 9 12" -- "$cur") )
                    return 0
                    ;;
                -p|--password|-s|--split)
                    return 0
                    ;;
            esac
            COMPREPLY=( $(compgen -f -- "$cur") )
            ;;
        extract)
            case "$prev" in
                -o|--output)
                    COMPREPLY=( $(compgen -d -- "$cur") )
                    return 0
                    ;;
                -p|--password)
                    return 0
                    ;;
            esac
            COMPREPLY=( $(compgen -f -- "$cur") )
            ;;
        bench)
            COMPREPLY=( $(compgen -W "matrix gate deflate zstd snappy" -- "$cur") )
            ;;
        list|inspect|test|salvage)
            COMPREPLY=( $(compgen -f -- "$cur") )
            ;;
    esac
}

complete -F _ttzip ttzip
