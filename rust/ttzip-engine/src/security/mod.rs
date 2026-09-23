// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! Security, sandbox isolation, path defense, and threat scanning modules.

pub use ttzip_crypto::security::acl;
pub use ttzip_crypto::security::blake3_defense;
pub use ttzip_crypto::security::ed25519_defense;
pub mod license;
pub use ttzip_crypto::security::mmap_defense;
pub use ttzip_crypto::security::path_sanitizer;
pub mod secure_extract;
pub mod system_defense;
pub use ttzip_archive::security::tar_defense;
pub use ttzip_archive::security::xz_defense;
pub use ttzip_archive::security::zip_defense;

#[cfg(test)]
mod tests;

pub use acl::*;
pub use blake3_defense::*;
pub use ed25519_defense::*;
pub use license::*;
pub use mmap_defense::*;
pub use path_sanitizer::*;
pub use secure_extract::*;
pub use system_defense::{
    AppcastSignatureGuard, BinaryDeltaBudgetOptions, BinaryDeltaMemoryBudgetGuard,
    DeltaMemoryPermit, PathTraversalOptions, PathTraversalProtectionGuard, SandboxEscapingGuard,
    SandboxEscapingOptions, SensitiveCredentialBuffer, SensitiveCredentialString,
    SystemDefenseError, SystemDefenseOptions, SystemPreflightReport, SystemSecurityPipeline,
    SystemUpdateRequest, TempDirectoryCleanupGuard, TempDirectoryGuard,
    DEFAULT_MAX_DELTA_EXPANSION_RATIO, DEFAULT_MAX_DELTA_INSTRUCTIONS,
    DEFAULT_MAX_DELTA_MEMORY_BUDGET, DEFAULT_MAX_DELTA_PATCH_SIZE,
};
pub use tar_defense::*;
pub use xz_defense::*;
pub use zip_defense::*;
