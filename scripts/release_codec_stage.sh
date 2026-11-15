# Copyright 2026 MetastudyLine. All rights reserved.
# SPDX-License-Identifier: BSD-3-Clause

RELEASE_CODEC_ARCHIVES=(libdeflate.a liblz4.a libzstd.a)

validate_release_codec_archive() {
    local archive="$1"
    local arch="$2"
    local actual_arch members

    if [ ! -f "${archive}" ]; then
        printf 'Missing release codec archive: %s\n' "${archive}" >&2
        return 1
    fi
    actual_arch="$(xcrun lipo -archs "${archive}")" || return 1
    if [ "${actual_arch}" != "${arch}" ]; then
        printf '%s has architecture %s; expected %s\n' "${archive}" "${actual_arch}" "${arch}" >&2
        return 1
    fi
    members="$(xcrun ar -t "${archive}" | awk '!/^__\.SYMDEF( |$)/ {count++} END {print count+0}')" || return 1
    xcrun otool -l "${archive}" | awk -v archive="${archive}" -v members="${members}" '
        function check_minimum(version, parts) {
            split(version, parts, ".")
            if ((platform != "1" && platform != "MACOS") ||
                version !~ /^[0-9]+[.][0-9]+([.][0-9]+)?$/ ||
                parts[1] > 14 || (parts[1] == 14 && (parts[2] > 0 || parts[3] > 0))) {
                printf "%s: %s has platform %s, minimum %s; expected macOS 14.0 or earlier\n",
                    archive, member, platform, version > "/dev/stderr"
                bad = 1
            }
            count++
        }
        /^[^[:space:]].*[(].*[)]:$/ { member = $0 }
        $1 == "cmd" { command = $2; platform = "" }
        command == "LC_BUILD_VERSION" && $1 == "platform" { platform = $2 }
        command == "LC_BUILD_VERSION" && $1 == "minos" { check_minimum($2) }
        command == "LC_VERSION_MIN_MACOSX" && $1 == "version" {
            platform = "MACOS"
            check_minimum($2)
        }
        END {
            if (count != members || count == 0) {
                printf "%s: expected %d macOS deployment load commands, found %d\n",
                    archive, members, count > "/dev/stderr"
                bad = 1
            }
            exit bad
        }'
}

prepare_release_codec_stages() {
    local root="${TTZIP_SYS_LIB_DIR:-}"
    local target arch stage archive
    RELEASE_CODEC_STAGES=()

    if [ -z "${root}" ] || [ ! -d "${root}" ]; then
        printf 'Set TTZIP_SYS_LIB_DIR to a directory with deployment-compatible libdeflate.a, liblz4.a and libzstd.a.\n' >&2
        return 1
    fi
    root="$(cd "${root}" && pwd)" || return 1
    for target in "$@"; do
        case "${target}" in
            aarch64-apple-darwin) arch=arm64 ;;
            x86_64-apple-darwin) arch=x86_64 ;;
            *) printf 'Unsupported macOS release target: %s\n' "${target}" >&2; return 1 ;;
        esac
        stage="${root}"
        if [ "$#" -gt 1 ] || [ -d "${root}/${arch}" ]; then
            stage="${root}/${arch}"
        fi
        for archive in "${RELEASE_CODEC_ARCHIVES[@]}"; do
            validate_release_codec_archive "${stage}/${archive}" "${arch}" || return 1
        done
        RELEASE_CODEC_STAGES+=("${stage}")
    done
}
