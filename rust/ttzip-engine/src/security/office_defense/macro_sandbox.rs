// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! Guard 4: Office Macro Sandbox, DDE Injection Interceptor, & External Rel Neutralizer.
//!
//! Enforces deterministic active content insulation for Office Open XML and legacy containers:
//! 1. Physical stripping and purging of `vbaProject.bin`, `activeX*.bin`, and macro streams.
//! 2. Interception of `=cmd|`, DDE / DDEAUTO, and formula-based command execution vectors.
//! 3. Neutralization of external UNC paths and remote template injection relationships.

use super::OfficeDefenseError;

/// Report detailing active content detections, stripped binaries, and neutralized relationships.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacroSanitizationReport {
    pub stripped_macro_files: Vec<String>,
    pub stripped_activex_files: Vec<String>,
    pub blocked_dde_formulas: usize,
    pub neutralized_external_rels: usize,
}

/// Guard enforcing macro purging, DDE injection interception, and external relationship safety.
#[derive(Debug, Clone, Default)]
pub struct OfficeMacroSandboxGuard {
    strip_macros: bool,
    block_dde_formulas: bool,
    neutralize_remote_rels: bool,
}

impl OfficeMacroSandboxGuard {
    /// Creates a new macro sandbox guard with default strict defense settings.
    pub fn new() -> Self {
        Self {
            strip_macros: true,
            block_dde_formulas: true,
            neutralize_remote_rels: true,
        }
    }

    /// Configures whether macros and ActiveX binaries should be physically purged.
    pub fn with_strip_macros(mut self, enabled: bool) -> Self {
        self.strip_macros = enabled;
        self
    }

    /// Configures whether DDE and formula injection payloads should be blocked.
    pub fn with_block_dde(mut self, enabled: bool) -> Self {
        self.block_dde_formulas = enabled;
        self
    }

    /// Configures whether remote external relationships (UNC, web templates) should be neutralized.
    pub fn with_neutralize_remote_rels(mut self, enabled: bool) -> Self {
        self.neutralize_remote_rels = enabled;
        self
    }

    /// Checks if a package entry path represents a dangerous macro or ActiveX binary.
    pub fn is_dangerous_entry_path(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/").to_ascii_lowercase();
        let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);

        // VBA Project binaries and data
        if file_name == "vbaproject.bin"
            || file_name == "vbadata.xml"
            || file_name == "vbaproject.signature"
            || file_name.ends_with(".vba")
            || file_name.ends_with(".bas")
            || file_name.ends_with(".cls")
        {
            return true;
        }

        // ActiveX controls and binaries
        if normalized.contains("/activex/")
            || (file_name.starts_with("activex") && file_name.ends_with(".bin"))
            || (file_name.starts_with("activex") && file_name.ends_with(".xml"))
        {
            return true;
        }

        false
    }

    /// Determines if an archive entry should be stripped during unpacking/rendering.
    pub fn should_strip_entry(&self, path: &str) -> bool {
        self.strip_macros && self.is_dangerous_entry_path(path)
    }

    /// Inspects a cell formula string, intercepting DDE, command execution, and malicious schemes.
    pub fn inspect_formula_security(&self, formula: &str) -> Result<(), OfficeDefenseError> {
        if !self.block_dde_formulas {
            return Ok(());
        }

        let trimmed = formula.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let upper = trimmed.to_ascii_uppercase();

        // 1. Check for DDE and DDEAUTO command triggers
        if upper.starts_with("=DDE(")
            || upper.starts_with("+DDE(")
            || upper.starts_with("-DDE(")
            || upper.starts_with("@DDE(")
            || upper.starts_with("=DDEAUTO(")
            || upper.starts_with("+DDEAUTO(")
            || upper.starts_with("-DDEAUTO(")
            || upper.starts_with("@DDEAUTO(")
        {
            return Err(OfficeDefenseError::DdeCommandBlocked {
                formula: formula.to_string(),
            });
        }

        // 2. Check for pipe-based command execution (e.g. =cmd|' /C calc'!A0)
        if upper.contains("CMD|")
            || upper.contains("POWERSHELL|")
            || upper.contains("MSHTA|")
            || upper.contains("CSCRIPT|")
            || upper.contains("WSCRIPT|")
            || upper.contains("REGSVR32|")
            || upper.contains("RUNDLL32|")
        {
            return Err(OfficeDefenseError::DdeCommandBlocked {
                formula: formula.to_string(),
            });
        }

        // 3. Check for Excel 4.0 macro functions and execution helpers
        if upper.contains("=EXEC(")
            || upper.contains("+EXEC(")
            || upper.contains("=SYSTEM(")
            || upper.contains("+SYSTEM(")
            || upper.contains("CALL(")
            || upper.contains("REGISTER(")
        {
            return Err(OfficeDefenseError::DangerousFormulaPayload {
                formula: formula.to_string(),
            });
        }

        // 4. Check for dangerous Hyperlink protocols (e.g. =HYPERLINK("powershell:...", "Click"))
        if upper.contains("HYPERLINK(") {
            let lower = trimmed.to_ascii_lowercase();
            if lower.contains("cmd:")
                || lower.contains("cmd.exe")
                || lower.contains("powershell:")
                || lower.contains("powershell.exe")
                || lower.contains("ms-msdt:")
                || lower.contains("ms-appx:")
                || lower.contains("cscript:")
                || lower.contains("wscript:")
                || lower.contains("certutil")
            {
                return Err(OfficeDefenseError::DangerousFormulaPayload {
                    formula: formula.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Inspects and sanitizes an OpenXML relationship target, neutralizing UNC and remote template injection.
    pub fn sanitize_relationship_target(
        &self,
        target: &str,
        target_mode: Option<&str>,
        rel_type: &str,
    ) -> Result<String, OfficeDefenseError> {
        let is_external = target_mode.map_or(false, |m| m.eq_ignore_ascii_case("External"));
        let lower = target.to_ascii_lowercase();

        // Check for UNC path (e.g. \\attacker.com\share or //attacker.com/share)
        if target.starts_with(r"\\") || target.starts_with("//") || lower.starts_with("smb:") {
            if self.neutralize_remote_rels {
                return Err(OfficeDefenseError::UncPathNeutralized {
                    target: target.to_string(),
                });
            }
        }

        // Check for dangerous URI schemes in external relationships (CVE-2022-30190 / Follina, remote templates)
        if is_external {
            if lower.starts_with("ms-msdt:")
                || lower.starts_with("ms-appx:")
                || lower.starts_with("javascript:")
                || lower.starts_with("data:")
                || lower.starts_with("file:")
            {
                return Err(OfficeDefenseError::DangerousRelationshipTarget {
                    target: target.to_string(),
                    rel_type: rel_type.to_string(),
                });
            }

            // Remote attached template injection (e.g. attachedTemplate pointing to http://*.dotm)
            if rel_type.contains("attachedTemplate")
                && (lower.starts_with("http://") || lower.starts_with("https://"))
            {
                if self.neutralize_remote_rels {
                    return Err(OfficeDefenseError::RemoteTemplateNeutralized {
                        target: target.to_string(),
                    });
                }
            }
        }

        Ok(target.to_string())
    }
}
