// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

import Foundation

extension ArchiveFormatStandardRegistry {
    
    /// Registers standard specifications for package, system, and extended container formats.
    public func registerPackagingSpecs() {
        // 21. CAB (Microsoft Cabinet)
        register(spec: ArchiveFormatStandardSpec(
            id: "cab",
            format: .cab,
            officialName: "Microsoft Cabinet (CAB) Format",
            standardCitations: [
                StandardCitation(
                    organization: "Microsoft Corporation",
                    standardNumber: "MS-CAB Specification",
                    title: "Cabinet File Architecture and Compression Specification",
                    canonicalURL: "https://learn.microsoft.com/en-us/previous-versions/bb267310(v=msdn.10)"
                )
            ],
            mimeType: "application/vnd.ms-cab-compressed",
            appleUTI: "com.microsoft.cab-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x4D, 0x53, 0x43, 0x46],
                    description: "MSCF Cabinet Header Signature"
                )
            ]
        ))

        // 22. CPIO
        register(spec: ArchiveFormatStandardSpec(
            id: "cpio",
            format: .cpio,
            officialName: "POSIX Portable CPIO Archive Format",
            standardCitations: [
                StandardCitation(
                    organization: "IEEE / The Open Group",
                    standardNumber: "POSIX.1-2017 / cpio",
                    title: "Portable Archive Format Specification (odc / newc)",
                    canonicalURL: "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cpio.html"
                )
            ],
            mimeType: "application/x-cpio",
            appleUTI: "public.cpio-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x30, 0x37, 0x30, 0x37, 0x30, 0x31],
                    description: "070701 SVR4 Portable Format Header"
                ),
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x30, 0x37, 0x30, 0x37, 0x30, 0x32],
                    description: "070702 SVR4 CRC Format Header"
                )
            ]
        ))

        // 23. AR (Unix Common Archive)
        register(spec: ArchiveFormatStandardSpec(
            id: "ar",
            format: .ar,
            officialName: "Unix Common Archive Format (ar / lib.a)",
            standardCitations: [
                StandardCitation(
                    organization: "AT&T / BSD",
                    standardNumber: "ar(5) Archive Format",
                    title: "Common Archive Format for Object Files and Static Libraries",
                    canonicalURL: "https://man.freebsd.org/cgi/man.cgi?query=ar&sektion=5"
                )
            ],
            mimeType: "application/x-archive",
            appleUTI: "public.archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x21, 0x3C, 0x61, 0x72, 0x63, 0x68, 0x3E, 0x0A],
                    description: "!<arch>\\n Unix Archive Signature"
                )
            ]
        ))

        // 24. DEB (Debian Software Package)
        register(spec: ArchiveFormatStandardSpec(
            id: "deb",
            format: .deb,
            officialName: "Debian Binary Package Format (deb)",
            standardCitations: [
                StandardCitation(
                    organization: "Debian Project",
                    standardNumber: "deb(5)",
                    title: "Debian Binary Package Format Specification",
                    canonicalURL: "https://manpages.debian.org/unstable/dpkg-dev/deb.5.en.html"
                )
            ],
            mimeType: "application/vnd.debian.binary-package",
            appleUTI: "org.debian.deb-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x21, 0x3C, 0x61, 0x72, 0x63, 0x68, 0x3E, 0x0A, 0x64, 0x65, 0x62, 0x69, 0x61, 0x6E],
                    description: "!<arch>\\ndebian Debian Binary Package Magic"
                )
            ]
        ))

        // 25. RPM (Red Hat Package Manager)
        register(spec: ArchiveFormatStandardSpec(
            id: "rpm",
            format: .rpm,
            officialName: "RPM Package Format v3.0 / v4.0",
            standardCitations: [
                StandardCitation(
                    organization: "RPM Project / Linux Foundation",
                    standardNumber: "LSB RPM Specification",
                    title: "RPM Package Format Specification",
                    canonicalURL: "https://rpm.org/documentation.html"
                )
            ],
            mimeType: "application/x-rpm",
            appleUTI: "com.redhat.rpm-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0xED, 0xAB, 0xEE, 0xDB],
                    description: "\\xED\\xAB\\xEE\\xDB RPM Lead Magic"
                )
            ]
        ))

        // 26. XAR (eXtensible ARchive / macOS PKG)
        register(spec: ArchiveFormatStandardSpec(
            id: "xar",
            format: .xar,
            officialName: "eXtensible Archive (XAR) Format",
            standardCitations: [
                StandardCitation(
                    organization: "Apple Inc. / Open Source",
                    standardNumber: "xar 1.6",
                    title: "eXtensible Archive Format Specification",
                    canonicalURL: "https://github.com/mackyle/xar"
                )
            ],
            mimeType: "application/x-xar",
            appleUTI: "com.apple.xar-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x78, 0x61, 0x72, 0x21],
                    description: "xar! Header Signature"
                )
            ]
        ))

        // 27. RAR (Roshal ARchive)
        register(spec: ArchiveFormatStandardSpec(
            id: "rar",
            format: .rar,
            officialName: "RAR Archive Format v4 / v5 Specification",
            standardCitations: [
                StandardCitation(
                    organization: "Alexander Roshal / RARLAB",
                    standardNumber: "RAR 5.0 Technical Notes",
                    title: "RAR 5.0 Archive File Format",
                    canonicalURL: "https://www.rarlab.com/technote.htm"
                )
            ],
            mimeType: "application/vnd.rar",
            appleUTI: "com.rarlab.rar-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00],
                    description: "Rar!\\x1A\\x07\\x00 RAR 4.x Header Magic"
                ),
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00],
                    description: "Rar!\\x1A\\x07\\x01\\x00 RAR 5.x Header Magic"
                )
            ],
            supportedEncryption: [
                EncryptionStandardSpec(
                    standardName: "RAR5 AES-256-CBC (PBKDF2-HMAC-SHA256)",
                    keyDerivationFunction: "PBKDF2-HMAC-SHA256 (2^17..2^24 cycles)",
                    cipher: "AES-256-CBC",
                    authenticationTag: "BLAKE2sp 256-bit"
                )
            ],
            supportsMultiVolume: true
        ))

        // 28. SquashFS
        register(spec: ArchiveFormatStandardSpec(
            id: "squashfs",
            format: .squashfs,
            officialName: "SquashFS Compressed Read-Only Filesystem",
            standardCitations: [
                StandardCitation(
                    organization: "Linux Kernel",
                    standardNumber: "SquashFS 4.0",
                    title: "SquashFS Read-Only Filesystem Specification",
                    canonicalURL: "https://docs.kernel.org/filesystems/squashfs.html"
                )
            ],
            mimeType: "application/vnd.squashfs",
            appleUTI: "public.squashfs-image",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x68, 0x73, 0x71, 0x73],
                    description: "hsqs SquashFS 4.0 Little-Endian Superblock"
                ),
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x73, 0x71, 0x73, 0x68],
                    description: "sqsh SquashFS 4.0 Big-Endian Superblock"
                )
            ]
        ))

        // 29. LZFSE
        register(spec: ArchiveFormatStandardSpec(
            id: "lzfse",
            format: .lzfse,
            officialName: "Apple LZFSE Compression Format",
            standardCitations: [
                StandardCitation(
                    organization: "Apple Inc.",
                    standardNumber: "LZFSE Open Source Reference",
                    title: "Lempel-Ziv Finite State Entropy Compression Algorithm",
                    canonicalURL: "https://github.com/lzfse/lzfse"
                )
            ],
            mimeType: "application/x-lzfse",
            appleUTI: "com.apple.lzfse-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x62, 0x76, 0x78, 0x24],
                    description: "bvx$ LZFSE Compressed Block Magic"
                ),
                ArchiveMagicSignature(
                    anchor: .head(offset: 0),
                    bytes: [0x62, 0x76, 0x78, 0x32],
                    description: "bvx2 LZFSE Compressed Block Magic"
                )
            ]
        ))

        // 30. LZH / LHA
        register(spec: ArchiveFormatStandardSpec(
            id: "lzh",
            format: .lzh,
            officialName: "LHA / LZH Compressed Archive Format",
            standardCitations: [
                StandardCitation(
                    organization: "Haruyasu Yoshizaki / LHA",
                    standardNumber: "LHA 2.55 Format Specification",
                    title: "LHA / LZH Compressed Archive Structure",
                    canonicalURL: "https://en.wikipedia.org/wiki/LHA_(file_format)"
                )
            ],
            mimeType: "application/x-lzh-compressed",
            appleUTI: "public.lzh-archive",
            magicSignatures: [
                ArchiveMagicSignature(
                    anchor: .head(offset: 2),
                    bytes: [0x2D, 0x6C, 0x68],
                    description: "-lh LHA Method Header (Offset 2)"
                )
            ]
        ))
    }
}
