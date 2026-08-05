//! Pins the crate's public surface. A compile failure here means an
//! exported item changed shape or disappeared; update this file only as a
//! deliberate API decision, never as a side effect of a refactor.

// Spelled-out signatures are this file's purpose; complexity is the pin.
#![allow(clippy::type_complexity)]

use rune_docs::error::Error;
use rune_docs::interop::{self, ConversionReport};
use rune_docs::sheet::Sheet;
use rune_docs::spec::{
    self, ArchiveOutput, ChangeState, ChangeSummary, ContextOutput, DiagnosticSeverity, ListSort,
    LiveTrees, MdschemaCheck, MergeSummary, ProposeOutput, ShowChangeOutput, ShowOutput,
    ShowSpecOutput, SpecDoctorOutput, SpecLayout, SpecRoot, SpecViolation, SpecificationSummary,
};
use std::path::{Path, PathBuf};

#[test]
fn the_root_resolution_surface_is_pinned() {
    let _: fn(&Path) -> Result<SpecRoot, Error> = spec::resolve_spec_root;
    let _: fn(&Path, &str) -> Result<SpecRoot, Error> = spec::resolve_spec_root_with;
    let _: fn(&Path) -> LiveTrees = spec::live_trees;
    let _: fn(&Path) -> Result<PathBuf, Error> = spec::changes_root;
    let _: fn(&Path) -> Result<PathBuf, Error> = spec::specs_root;
    let _: fn(&Path) -> Result<PathBuf, Error> = spec::spec_base;
    let _: fn(fn(&Path) -> Result<Option<String>, String>) -> bool = spec::set_root_config_lookup;
    let _ = SpecLayout::Native;
    let _ = LiveTrees {
        native: false,
        openspec: false,
    };
}

#[test]
fn the_output_surface_is_pinned() {
    let _: fn(&str, &str, &[String], bool) -> Result<ProposeOutput, Error> = spec::propose_output;
    let _: fn(&str, ListSort) -> Result<Vec<ChangeSummary>, Error> = spec::sorted_changes;
    let _: fn(&str, &str) -> Result<ContextOutput, Error> = spec::context_output;
    let _: fn(&str, &str) -> Result<ShowOutput, Error> = spec::show_output;
    let _: fn(&str, &str, bool, bool) -> Result<ArchiveOutput, Error> = spec::archive_output;
    let _: fn(&str, Option<&str>, MdschemaCheck) -> Result<Vec<SpecViolation>, Error> =
        spec::validate_output;
    let _: fn(&str) -> Result<SpecDoctorOutput, Error> = spec::doctor_output;
    let _: fn(&Path) -> Result<Vec<ChangeSummary>, Error> = spec::scan_changes;
    let _: fn(&Path) -> Result<Vec<SpecificationSummary>, Error> = spec::scan_specifications;
    let _: fn(&Path, MdschemaCheck) -> Result<Vec<SpecViolation>, Error> = spec::validate_spec_tree;
    let _: fn(&ArchiveOutput) -> &[String] = ArchiveOutput::warnings;
    let _: fn(&SpecDoctorOutput) -> bool = SpecDoctorOutput::has_errors;
    let _: fn(&SpecViolation) -> bool = SpecViolation::is_error;
    let _ = ChangeState::Draft;
    let _ = DiagnosticSeverity::Warning;
    let _ = MergeSummary::default();
    let _ = ListSort::Progress;
}

#[test]
fn the_render_surface_is_pinned() {
    let _: fn(&ProposeOutput) -> String = spec::render_propose;
    let _: fn(&[ChangeSummary], &Sheet) -> String = spec::render_change_list;
    let _: fn(&[SpecificationSummary]) -> String = spec::render_specification_list;
    let _: fn(&ContextOutput) -> String = spec::render_context;
    let _: fn(&ShowChangeOutput) -> String = spec::render_show_change;
    let _: fn(&ShowSpecOutput, &str) -> String = spec::render_show_specification;
    let _: fn(&ArchiveOutput) -> String = spec::render_archive;
    let _: fn(&[SpecViolation]) -> String = spec::render_diagnostics;
    let _: fn(&SpecDoctorOutput) -> String = spec::render_doctor;
    // `render_json` is generic over `Serialize`; instantiating it here pins
    // its shape.
    let rendered = spec::render_json(&Vec::<SpecViolation>::new()).unwrap();
    assert_eq!(rendered, "[]");
}

#[test]
fn the_interop_surface_is_pinned() {
    let _: fn(&str) -> Result<ConversionReport, Error> = interop::export_openspec_output;
    let _: fn(&str) -> Result<ConversionReport, Error> = interop::import_openspec_output;
    let _: fn(&str, &str) -> Result<ConversionReport, Error> = interop::import_openspec_into_output;
    let _: fn(&ConversionReport, &Sheet) -> String = interop::render_report;
    let _: fn(&ConversionReport) -> Result<String, Error> = interop::render_report_json;
}

/// The printing command functions the CLI has already migrated away from;
/// they stay until every consumer compiles against the output-returning
/// forms, then their removal updates this test as a deliberate API change.
#[test]
fn the_printing_wrappers_are_pinned() {
    let _: fn(&str, &str, &[String], bool, bool) -> Result<i32, Error> = spec::propose;
    let _: fn(&str, bool, ListSort, bool) -> Result<i32, Error> = spec::list;
    let _: fn(&str, &str, bool) -> Result<i32, Error> = spec::show;
    let _: fn(&str, &str, bool) -> Result<i32, Error> = spec::context;
    let _: fn(&str, bool) -> Result<i32, Error> = spec::doctor;
    let _: fn(&str, Option<&str>, bool, MdschemaCheck) -> Result<i32, Error> = spec::validate;
    let _: fn(&str, &str, bool, bool, bool) -> Result<i32, Error> = spec::archive;
    let _: fn(&str, bool) -> Result<i32, Error> = interop::export_openspec;
    let _: fn(&str, bool) -> Result<i32, Error> = interop::import_openspec;
    let _: fn(&str, &str, bool) -> Result<i32, Error> = interop::import_openspec_into;
}
