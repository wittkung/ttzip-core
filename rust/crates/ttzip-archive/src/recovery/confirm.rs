// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Archive-layer password confirmation.
//!
//! The archive layer owns the *content-level* proof that a candidate is the
//! password of an archive: it selects one encrypted unit, decrypts it with the
//! engine's real extraction entry, and compares the result against the integrity
//! credential the archive itself publishes.
//!
//! # What this layer does not claim
//!
//! * A rejection means **"the candidate was not accepted"**, not "the password is
//!   wrong". A tampered ciphertext payload fails the very same credential, and the
//!   archive layer cannot separate the two.
//! * A budget limit, a missing credential and a damaged container are reported as
//!   their own verdicts, so they can never be presented as a rejected password or
//!   as an exhausted search.
//! * The confirmation proves **one** encrypted unit only. It says nothing about the
//!   other entries of a multi-password archive.
//!
//! # Relationship to the cheap screen
//!
//! The cheap screening stage lives in the crypto crate and can only *eliminate*
//! candidates. This layer consumes the engine's real decrypt, decompress, CRC and
//! HMAC entries; it never re-implements them.

use crate::recovery::target::{
    select_seven_zip_unit, select_seven_zip_unit_at, select_zip_unit, select_zip_unit_at,
    ConfirmationTarget, IntegrityCredential, ScreeningMaterial, SelectedUnit,
};
use crate::recovery::verdict::{ConfirmationVerdict, PreparationError};
use crate::sevenz::decoder::SevenZArchive;
use crate::sevenz::decoder::stream::extract_single_entry_bounded;
use crate::types::TTZipStatus;
use crate::zip::reader::ZipArchive;

/// Default ceiling for the bytes one confirmation may materialize.
const DEFAULT_MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// Default ceiling for the bytes one confirmation may decode inside a 7z solid
/// folder before it reaches the target entry.
const DEFAULT_MAX_PRECEDING_BYTES: u64 = 64 * 1024 * 1024;

/// Budget bounding the work a single confirmation attempt may spend.
///
/// The budget is a property of the attempt, not of the candidate: exhausting it
/// yields [`ConfirmationVerdict::BudgetExhausted`], never a rejection.
///
/// # What the budget does not promise
///
/// The budget counts **bytes**, not time. It does not bound the wall-clock cost of a
/// key derivation: a 7z candidate pays its SHA-256 derivation before any byte is
/// decoded, and that cost scales with the archive's cycle exponent and with the
/// device. A caller that needs a time bound has to measure it and apply it
/// separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmationBudget {
    /// Maximum number of decompressed bytes one confirmation may materialize.
    ///
    /// Zero means that no entry may be materialized at all, so every confirmation
    /// against a non-empty entry runs into the budget.
    pub max_entry_bytes: u64,
    /// Maximum number of bytes that may be decoded inside a 7z solid folder before
    /// the target entry is reached.
    ///
    /// `Some(0)` means that **no** preceding decode is allowed: the target entry has
    /// to be the first stream of its folder. `None` means unbounded. The distinction
    /// is explicit in the type, so the archive layer never has to borrow a sentinel
    /// value of the lower layer to express "unbounded".
    pub max_preceding_bytes: Option<u64>,
}

impl Default for ConfirmationBudget {
    fn default() -> Self {
        Self {
            max_entry_bytes: DEFAULT_MAX_ENTRY_BYTES,
            max_preceding_bytes: Some(DEFAULT_MAX_PRECEDING_BYTES),
        }
    }
}

/// Outcome of a candidate search over one confirmation target.
///
/// * `Ok(Some(_))` — a candidate was confirmed.
/// * `Ok(None)` — every candidate was rejected, so the search was genuinely
///   exhausted.
/// * `Err(_)` — the search stopped for a reason that is **not** about the candidate:
///   a budget limit, an unsupported format, a damaged container or a missing
///   credential. The verdict is preserved so that a caller can tell those apart from
///   an exhausted search, instead of reading them as "no password found".
pub type SearchOutcome<'c> = Result<Option<&'c str>, ConfirmationVerdict>;

/// The parsed archive that a confirmation draws its evidence from.
enum Container<'a> {
    /// A ZIP container.
    Zip(ZipArchive<'a>),
    /// A 7z container together with the bytes it was parsed from.
    SevenZip {
        /// The parsed 7z archive.
        ///
        /// Boxed because the parsed 7z metadata is far larger than the ZIP entry
        /// table; keeping it on the heap holds the enum itself compact.
        archive: Box<SevenZArchive<'a>>,
        /// The raw archive bytes, needed for the bounded extraction entry.
        data: &'a [u8],
    },
}

/// Archive-layer password confirmation for one encrypted unit of one archive.
///
/// The container is parsed once and the confirmation unit is selected once, so the
/// same instance can confirm many candidates without re-parsing anything.
pub struct PasswordConfirmation<'a> {
    container: Container<'a>,
    unit: SelectedUnit,
    budget: ConfirmationBudget,
}

impl<'a> PasswordConfirmation<'a> {
    /// Prepares a confirmation for a ZIP archive.
    ///
    /// Fails with [`PreparationError::ArchiveDamaged`] when the container cannot be
    /// parsed, with [`PreparationError::NoEncryptedUnit`] when it holds no encrypted
    /// entry, and with [`PreparationError::NoCredential`] when its encrypted entries
    /// carry no integrity credential.
    pub fn for_zip(data: &'a [u8], budget: ConfirmationBudget) -> Result<Self, PreparationError> {
        let zip = ZipArchive::open_slice(data).map_err(|_| PreparationError::ArchiveDamaged)?;
        let unit = select_zip_unit(&zip, data)?;
        Ok(Self {
            container: Container::Zip(zip),
            unit,
            budget,
        })
    }

    /// Prepares a confirmation for a 7z archive.
    pub fn for_seven_zip(
        data: &'a [u8],
        budget: ConfirmationBudget,
    ) -> Result<Self, PreparationError> {
        let archive =
            SevenZArchive::open_slice(data).map_err(|_| PreparationError::ArchiveDamaged)?;
        let unit = select_seven_zip_unit(&archive)?;
        Ok(Self {
            container: Container::SevenZip {
                archive: Box::new(archive),
                data,
            },
            unit,
            budget,
        })
    }

    /// Prepares a confirmation for one named entry of a ZIP archive.
    ///
    /// The automatic selection takes the first entry that carries a credential. A
    /// caller that wants to confirm against a different entry can name it here.
    pub fn for_zip_entry(
        data: &'a [u8],
        budget: ConfirmationBudget,
        entry_index: usize,
    ) -> Result<Self, PreparationError> {
        let zip = ZipArchive::open_slice(data).map_err(|_| PreparationError::ArchiveDamaged)?;
        let unit = select_zip_unit_at(&zip, data, entry_index)?;
        Ok(Self {
            container: Container::Zip(zip),
            unit,
            budget,
        })
    }

    /// Prepares a confirmation for one named entry of a 7z archive.
    ///
    /// This matters for solid archives: an entry that is not the first stream of its
    /// folder can only be reached by decoding the streams in front of it, and that
    /// preceding decode is what [`ConfirmationBudget::max_preceding_bytes`] bounds.
    pub fn for_seven_zip_entry(
        data: &'a [u8],
        budget: ConfirmationBudget,
        entry_index: usize,
    ) -> Result<Self, PreparationError> {
        let archive =
            SevenZArchive::open_slice(data).map_err(|_| PreparationError::ArchiveDamaged)?;
        let unit = select_seven_zip_unit_at(&archive, entry_index)?;
        Ok(Self {
            container: Container::SevenZip {
                archive: Box::new(archive),
                data,
            },
            unit,
            budget,
        })
    }

    /// Prepares a confirmation for an archive of a detected format.
    pub fn for_archive(
        data: &'a [u8],
        budget: ConfirmationBudget,
    ) -> Result<Self, PreparationError> {
        if data.starts_with(b"7z\xBC\xAF\x27\x1C") {
            return Self::for_seven_zip(data, budget);
        }
        Self::for_zip(data, budget)
    }

    /// The encrypted unit this confirmation proves.
    #[inline]
    pub fn target(&self) -> ConfirmationTarget {
        self.unit.target
    }

    /// The integrity credential a candidate has to satisfy.
    #[inline]
    pub fn credential(&self) -> IntegrityCredential {
        self.unit.credential
    }

    /// The cheap-check material of the selected unit, when the format provides one.
    ///
    /// 7z yields `None`: the key derivation dominates the cost of a 7z attempt and no
    /// fixed magic can stand in for a content-level check, so a 7z candidate has to
    /// be confirmed rather than screened.
    #[inline]
    pub fn screening_material(&self) -> Option<ScreeningMaterial> {
        self.unit.screening.clone()
    }

    /// Confirms one candidate against the selected encrypted unit.
    pub fn confirm(&self, candidate: &str) -> ConfirmationVerdict {
        match &self.container {
            Container::Zip(zip) => self.confirm_zip(zip, candidate),
            Container::SevenZip { archive, data } => self.confirm_seven_zip(archive, data, candidate),
        }
    }

    /// Confirms candidates in order and returns the first one that is accepted.
    ///
    /// A candidate that is **not accepted** never stops the search; the search
    /// continues with the next candidate. Any other verdict — a budget limit, an
    /// unsupported format, a damaged container or a missing credential — stops the
    /// search and is returned as the error, because continuing would be pointless
    /// work and because such a verdict says nothing about the candidate.
    pub fn confirm_first<'c, I>(&self, candidates: I) -> SearchOutcome<'c>
    where
        I: IntoIterator<Item = &'c str>,
    {
        for candidate in candidates {
            match self.confirm(candidate) {
                ConfirmationVerdict::Confirmed => return Ok(Some(candidate)),
                ConfirmationVerdict::NotAccepted => continue,
                other => return Err(other),
            }
        }
        Ok(None)
    }

    /// Confirms one candidate against a ZIP entry.
    fn confirm_zip(&self, zip: &ZipArchive, candidate: &str) -> ConfirmationVerdict {
        let ConfirmationTarget::ZipEntry { index } = self.unit.target else {
            return ConfirmationVerdict::ArchiveDamaged;
        };
        let Some(entry) = zip.entries().get(index) else {
            return ConfirmationVerdict::ArchiveDamaged;
        };
        if entry.uncompressed_size > self.budget.max_entry_bytes {
            return ConfirmationVerdict::BudgetExhausted;
        }
        match zip.extract_entry_bytes(index, Some(candidate)) {
            Ok(_) => ConfirmationVerdict::Confirmed,
            Err(TTZipStatus::ErrInvalidPassword) => ConfirmationVerdict::NotAccepted,
            Err(TTZipStatus::ErrOutOfMemory) => ConfirmationVerdict::BudgetExhausted,
            Err(TTZipStatus::ErrArchiveInitFailed) | Err(TTZipStatus::ErrUnsupportedFeature) => {
                // The entry uses a compression method or a feature the engine cannot
                // decode. That is a limitation of the archive, not of the candidate.
                ConfirmationVerdict::Unsupported
            }
            Err(TTZipStatus::ErrCorruptHeader) | Err(TTZipStatus::ErrExtractionFailed) => {
                // The payload of an encrypted unit may have been tampered with. The
                // container layout was already checked while preparing the
                // confirmation, so this is a content-level failure and the archive
                // layer cannot call it a proven wrong password.
                ConfirmationVerdict::NotAccepted
            }
            Err(_) => ConfirmationVerdict::ArchiveDamaged,
        }
    }

    /// Translates the archive-layer preceding-byte budget into the lower layer's limit.
    ///
    /// The lower layer treats a limit of zero as "no limit", so the archive layer
    /// never passes zero: an explicit zero budget is checked here and means "no
    /// preceding decode at all", while an unbounded budget is expressed as the
    /// largest possible limit rather than by borrowing the lower sentinel.
    fn seven_zip_preceding_budget(&self) -> u64 {
        match self.budget.max_preceding_bytes {
            // A zero budget that would have to forbid a preceding decode was
            // already reported as exhausted by the caller. With no preceding decode
            // ahead of the target every limit is equivalent, and the largest one
            // keeps the lower layer's sentinel unused.
            Some(0) => u64::MAX,
            Some(limit) => limit,
            None => u64::MAX,
        }
    }

    /// Confirms one candidate against a 7z entry.
    fn confirm_seven_zip(
        &self,
        archive: &SevenZArchive,
        data: &'a [u8],
        candidate: &str,
    ) -> ConfirmationVerdict {
        let ConfirmationTarget::SevenZipEntry { index, .. } = self.unit.target else {
            return ConfirmationVerdict::ArchiveDamaged;
        };
        let Some(loc) = archive.seek_index().get_by_index(index) else {
            return ConfirmationVerdict::ArchiveDamaged;
        };
        if loc.uncompressed_size > self.budget.max_entry_bytes {
            return ConfirmationVerdict::BudgetExhausted;
        }
        if self.budget.max_preceding_bytes == Some(0) && loc.offset_in_folder > 0 {
            // A zero preceding budget forbids any decode in front of the target
            // entry, and this entry sits behind one.
            return ConfirmationVerdict::BudgetExhausted;
        }
        let result = extract_single_entry_bounded(
            data,
            archive.info(),
            archive.seek_index(),
            index,
            Some(candidate),
            self.seven_zip_preceding_budget(),
        );
        match result {
            Ok(_) => ConfirmationVerdict::Confirmed,
            Err(TTZipStatus::ErrInvalidPassword) => ConfirmationVerdict::NotAccepted,
            Err(TTZipStatus::ErrSolidBudgetExceeded) | Err(TTZipStatus::ErrOutOfMemory) => {
                ConfirmationVerdict::BudgetExhausted
            }
            Err(TTZipStatus::ErrUnsupportedFeature)
            | Err(TTZipStatus::ErrArchiveInitFailed) => ConfirmationVerdict::Unsupported,
            Err(TTZipStatus::ErrCorruptHeader) | Err(TTZipStatus::ErrExtractionFailed) => {
                // Same ambiguity as the ZIP path: the ciphertext of the folder may
                // have been tampered with.
                ConfirmationVerdict::NotAccepted
            }
            Err(_) => ConfirmationVerdict::ArchiveDamaged,
        }
    }
}
