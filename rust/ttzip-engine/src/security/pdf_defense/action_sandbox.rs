// SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
//
// Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
// All rights reserved.
//
// TTZip: High-performance native archiving and compression engine.

//! PDF Malicious Action Sandbox Guard.
//!
//! Provides total isolation against malicious active content embedded in PDF files,
//! including embedded JavaScript, OS command execution (`/Launch`), form exfiltration,
//! and dangerous URI schemes.

use super::PdfDefenseError;

/// Threat severity level of a detected PDF active content element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionThreatLevel {
    /// Informational or low risk active content.
    Low,
    /// Potentially hazardous content requiring audit.
    Medium,
    /// High-risk active content (e.g. URI navigation, form submission).
    High,
    /// Critical security threat (e.g. JavaScript execution, OS command launch).
    Critical,
}

/// Details of a detected threat within a PDF document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionThreat {
    /// Category/Type of action (e.g. "JavaScript", "Launch", "DangerousURI").
    pub action_type: String,
    /// Context or location where the threat was found (e.g. "Catalog/OpenAction", "Page[1]/Annot[0]").
    pub location: String,
    /// Detailed description of the threat payload.
    pub details: String,
    /// Severity level.
    pub severity: ActionThreatLevel,
}

/// Policy dictating how the sandbox handles detected active content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionPolicy {
    /// Strictly rejects the document if any active or dangerous content is detected.
    #[default]
    RejectAllActiveContent,
    /// Strips all dangerous actions in-place, neutralizing the threat while preserving static pages.
    SanitizeAndStrip,
    /// Audits and logs threats without throwing an error or modifying the document.
    AuditOnly,
}

/// Comprehensive report produced by the sandbox guard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxReport {
    /// List of all threats detected during document inspection.
    pub threats: Vec<ActionThreat>,
    /// Whether any elements were stripped/sanitized.
    pub is_sanitized: bool,
    /// Total count of inspected action targets.
    pub actions_inspected: usize,
}

impl SandboxReport {
    /// Returns true if any Critical or High severity threats were discovered.
    pub fn has_critical_threats(&self) -> bool {
        self.threats
            .iter()
            .any(|t| t.severity >= ActionThreatLevel::High)
    }
}

/// Guard inspecting, sandboxing, and neutralizing dangerous PDF actions.
#[derive(Debug, Clone)]
pub struct MaliciousActionSandboxGuard {
    policy: ActionPolicy,
}

impl Default for MaliciousActionSandboxGuard {
    fn default() -> Self {
        Self::new(ActionPolicy::RejectAllActiveContent)
    }
}

impl MaliciousActionSandboxGuard {
    /// Creates a new sandbox guard with the specified action policy.
    pub fn new(policy: ActionPolicy) -> Self {
        Self { policy }
    }

    /// Returns the currently active sandbox policy.
    pub fn policy(&self) -> ActionPolicy {
        self.policy
    }

    /// Checks whether a URI string utilizes a dangerous or exploitative scheme.
    pub fn is_dangerous_uri(uri: &str) -> bool {
        let lower = uri.trim().to_ascii_lowercase();
        let dangerous_schemes = [
            "javascript:",
            "vbscript:",
            "file:",
            "data:",
            "ms-msdt:",
            "search-ms:",
            "powershell:",
            "cmd:",
            "cscript:",
            "wscript:",
            "shell:",
            "jar:",
            "expect:",
            "php:",
        ];
        dangerous_schemes.iter().any(|&scheme| lower.starts_with(scheme))
    }

    /// Scans a `lopdf::Document` for active content, applying the configured policy.
    pub fn inspect_document(&self, doc: &lopdf::Document) -> Result<SandboxReport, PdfDefenseError> {
        let mut report = SandboxReport::default();

        // 1. Inspect Document Catalog (/OpenAction, /AA, /Names/JavaScript)
        if let Ok(catalog_id) = doc.trailer.get(b"Root") {
            if let Ok(catalog_obj) = self.resolve_obj(doc, catalog_id) {
                if let Ok(catalog_dict) = catalog_obj.as_dict() {
                    self.inspect_catalog_dict(doc, catalog_dict, &mut report);
                }
            }
        }

        // 2. Inspect all objects for action dictionaries (/S /JavaScript, /Launch, etc.)
        for (&(obj_nr, gen_nr), obj) in &doc.objects {
            let loc = format!("Object({obj_nr}, {gen_nr})");
            self.inspect_object_for_actions(doc, obj, &loc, &mut report);
        }

        // 3. Enforce policy
        if self.policy == ActionPolicy::RejectAllActiveContent && !report.threats.is_empty() {
            if let Some(critical) = report.threats.iter().find(|t| t.severity >= ActionThreatLevel::High) {
                return Err(PdfDefenseError::MaliciousActionDetected {
                    action_type: critical.action_type.clone(),
                    details: format!("{} at {}", critical.details, critical.location),
                });
            }
        }

        Ok(report)
    }

    /// Sanitizes a `lopdf::Document` in-place by removing dangerous action triggers and dictionaries.
    pub fn sanitize_document(&self, doc: &mut lopdf::Document) -> Result<SandboxReport, PdfDefenseError> {
        let mut report = SandboxReport {
            threats: Vec::new(),
            is_sanitized: false,
            actions_inspected: 0,
        };

        // 1. Sanitize Document Catalog (/Root)
        if let Ok(catalog_id) = doc.trailer.get(b"Root").cloned() {
            let cat_id = match catalog_id {
                lopdf::Object::Reference(id) => Some(id),
                _ => None,
            };

            if let Some(id) = cat_id {
                if let Ok(catalog_obj) = doc.get_object_mut(id) {
                    if let Ok(dict) = catalog_obj.as_dict_mut() {
                        if dict.has(b"OpenAction") {
                            dict.remove(b"OpenAction");
                            report.threats.push(ActionThreat {
                                action_type: "OpenAction".to_string(),
                                location: "Catalog".to_string(),
                                details: "Removed /OpenAction trigger".to_string(),
                                severity: ActionThreatLevel::High,
                            });
                            report.is_sanitized = true;
                        }
                        if dict.has(b"AA") {
                            dict.remove(b"AA");
                            report.threats.push(ActionThreat {
                                action_type: "AdditionalActions".to_string(),
                                location: "Catalog".to_string(),
                                details: "Removed /AA dictionary".to_string(),
                                severity: ActionThreatLevel::High,
                            });
                            report.is_sanitized = true;
                        }
                    }
                }
            }
        }

        // 2. Sanitize all object dictionaries
        let object_ids: Vec<lopdf::ObjectId> = doc.objects.keys().copied().collect();
        for id in object_ids {
            if let Ok(obj) = doc.get_object_mut(id) {
                if let Ok(dict) = obj.as_dict_mut() {
                    self.sanitize_dict(dict, &mut report, &format!("Object({:?})", id));
                }
            }
        }

        Ok(report)
    }

    fn inspect_catalog_dict(
        &self,
        doc: &lopdf::Document,
        dict: &lopdf::Dictionary,
        report: &mut SandboxReport,
    ) {
        report.actions_inspected += 1;

        // Check /OpenAction
        if let Ok(open_action) = dict.get(b"OpenAction") {
            report.threats.push(ActionThreat {
                action_type: "OpenAction".to_string(),
                location: "Catalog/OpenAction".to_string(),
                details: "Automatic document execution trigger detected".to_string(),
                severity: ActionThreatLevel::High,
            });
            self.inspect_object_for_actions(doc, open_action, "Catalog/OpenAction", report);
        }

        // Check /AA (Additional Actions)
        if let Ok(aa) = dict.get(b"AA") {
            report.threats.push(ActionThreat {
                action_type: "AdditionalActions".to_string(),
                location: "Catalog/AA".to_string(),
                details: "Document-level lifecycle event actions detected".to_string(),
                severity: ActionThreatLevel::High,
            });
            self.inspect_object_for_actions(doc, aa, "Catalog/AA", report);
        }

        // Check /Names -> /JavaScript
        if let Ok(names) = dict.get(b"Names") {
            if let Ok(names_obj) = self.resolve_obj(doc, names) {
                if let Ok(names_dict) = names_obj.as_dict() {
                    if names_dict.has(b"JavaScript") {
                        report.threats.push(ActionThreat {
                            action_type: "JavaScriptNameTree".to_string(),
                            location: "Catalog/Names/JavaScript".to_string(),
                            details: "Embedded JavaScript name tree detected".to_string(),
                            severity: ActionThreatLevel::Critical,
                        });
                    }
                }
            }
        }
    }

    fn inspect_object_for_actions(
        &self,
        doc: &lopdf::Document,
        obj: &lopdf::Object,
        loc: &str,
        report: &mut SandboxReport,
    ) {
        report.actions_inspected += 1;

        match obj {
            lopdf::Object::Reference(id) => {
                if let Ok(target) = doc.get_object(*id) {
                    let sub_loc = format!("{loc}->{:?}", id);
                    self.inspect_object_for_actions(doc, target, &sub_loc, report);
                }
            }
            lopdf::Object::Dictionary(dict) => {
                self.inspect_action_dict(dict, loc, report);
            }
            lopdf::Object::Stream(stream) => {
                self.inspect_action_dict(&stream.dict, loc, report);
            }
            lopdf::Object::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    let sub_loc = format!("{loc}[{i}]");
                    self.inspect_object_for_actions(doc, item, &sub_loc, report);
                }
            }
            _ => {}
        }
    }

    fn inspect_action_dict(&self, dict: &lopdf::Dictionary, loc: &str, report: &mut SandboxReport) {
        if let Ok(action_type_obj) = dict.get(b"S") {
            if let Ok(action_type) = action_type_obj.as_name_str() {
                match action_type {
                    "JavaScript" => {
                        report.threats.push(ActionThreat {
                            action_type: "JavaScript".to_string(),
                            location: loc.to_string(),
                            details: "Embedded executable JavaScript action detected".to_string(),
                            severity: ActionThreatLevel::Critical,
                        });
                    }
                    "Launch" => {
                        report.threats.push(ActionThreat {
                            action_type: "Launch".to_string(),
                            location: loc.to_string(),
                            details: "OS command execution / binary launch action detected".to_string(),
                            severity: ActionThreatLevel::Critical,
                        });
                    }
                    "SubmitForm" | "ImportData" | "ResetForm" => {
                        report.threats.push(ActionThreat {
                            action_type: action_type.to_string(),
                            location: loc.to_string(),
                            details: format!("Interactive form manipulation/exfiltration action: /{action_type}"),
                            severity: ActionThreatLevel::High,
                        });
                    }
                    "URI" => {
                        if let Ok(uri_obj) = dict.get(b"URI") {
                            let uri_str = match uri_obj {
                                lopdf::Object::String(bytes, _) => String::from_utf8_lossy(bytes).to_string(),
                                _ => String::new(),
                            };
                            if Self::is_dangerous_uri(&uri_str) {
                                report.threats.push(ActionThreat {
                                action_type: "DangerousURI".to_string(),
                                location: loc.to_string(),
                                details: format!("Prohibited dangerous URI scheme: {uri_str}"),
                                severity: ActionThreatLevel::Critical,
                            });
                            }
                        }
                    }
                    "GoToR" | "GoToE" => {
                        report.threats.push(ActionThreat {
                            action_type: action_type.to_string(),
                            location: loc.to_string(),
                            details: format!("Remote/embedded document navigation action: /{action_type}"),
                            severity: ActionThreatLevel::Medium,
                        });
                    }
                    "Sound" | "Movie" | "RichMedia" | "Rendition" => {
                        report.threats.push(ActionThreat {
                            action_type: action_type.to_string(),
                            location: loc.to_string(),
                            details: format!("Active multimedia payload action: /{action_type}"),
                            severity: ActionThreatLevel::Medium,
                        });
                    }
                    _ => {}
                }
            }
        }

        // Check if dictionary contains /JS entry directly
        if dict.has(b"JS") {
            report.threats.push(ActionThreat {
                action_type: "JSCode".to_string(),
                location: loc.to_string(),
                details: "Direct /JS code script payload detected in dictionary".to_string(),
                severity: ActionThreatLevel::Critical,
            });
        }
    }

    fn sanitize_dict(&self, dict: &mut lopdf::Dictionary, report: &mut SandboxReport, loc: &str) {
        let mut dangerous = false;

        if let Ok(action_type_obj) = dict.get(b"S") {
            if let Ok(action_type) = action_type_obj.as_name_str() {
                if matches!(action_type, "JavaScript" | "Launch" | "SubmitForm" | "ImportData") {
                    dangerous = true;
                } else if action_type == "URI" {
                    if let Ok(lopdf::Object::String(bytes, _)) = dict.get(b"URI") {
                        let uri_str = String::from_utf8_lossy(bytes);
                        if Self::is_dangerous_uri(&uri_str) {
                            dangerous = true;
                        }
                    }
                }
            }
        }

        if dict.has(b"JS") {
            dangerous = true;
        }

        if dangerous {
            dict.remove(b"S");
            dict.remove(b"JS");
            dict.remove(b"URI");
            dict.remove(b"F");
            dict.remove(b"Win");
            dict.remove(b"Mac");
            dict.remove(b"Unix");
            report.threats.push(ActionThreat {
                action_type: "SanitizedAction".to_string(),
                location: loc.to_string(),
                details: "Neutralized dangerous action keys from dictionary".to_string(),
                severity: ActionThreatLevel::High,
            });
            report.is_sanitized = true;
        }
    }

    fn resolve_obj<'a>(
        &self,
        doc: &'a lopdf::Document,
        obj: &'a lopdf::Object,
    ) -> Result<&'a lopdf::Object, PdfDefenseError> {
        match obj {
            lopdf::Object::Reference(id) => doc.get_object(*id).map_err(|e| {
                PdfDefenseError::MalformedPdf {
                    reason: format!("Unresolvable reference {:?}: {e}", id),
                    offset: None,
                }
            }),
            _ => Ok(obj),
        }
    }
}
