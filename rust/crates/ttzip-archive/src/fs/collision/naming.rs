// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Keep Both naming logic.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use crate::fs::collision::ItemKind;

/// Generates a "Keep Both" candidate name.
/// `<stem> N<.ext>`, dotfiles have no extension, directories never split.
pub fn candidate(name: &OsStr, kind: ItemKind, n: u32) -> OsString {
    let mut out = OsString::new();

    if kind == ItemKind::Directory {
        out.push(name);
        out.push(format!(" {}", n));
        return out;
    }

    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or(name);
    let ext = path.extension();

    out.push(stem);
    out.push(format!(" {}", n));

    if let Some(e) = ext {
        out.push(".");
        out.push(e);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_candidate() {
        let cases = vec![
            ("name.txt", ItemKind::File, 2, "name 2.txt"),
            ("name 2.txt", ItemKind::File, 2, "name 2 2.txt"),
            (".bashrc", ItemKind::File, 2, ".bashrc 2"),
            ("archive.tar.gz", ItemKind::File, 2, "archive.tar 2.gz"),
            ("folder", ItemKind::Directory, 2, "folder 2"),
            ("folder.ext", ItemKind::Directory, 2, "folder.ext 2"),
        ];

        for (input, kind, n, expected) in cases {
            let result = candidate(OsStr::new(input), kind, n);
            assert_eq!(result, OsString::from(expected));
        }
    }
}
