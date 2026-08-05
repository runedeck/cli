use super::*;

fn make_artifact(kind: &str, name: &str, module: &str) -> ArtifactView {
    ArtifactView {
        name: name.to_string(),
        kind: kind.to_string(),
        module: module.to_string(),
        relative_path: format!("{kind}/{name}.md"),
        description: String::new(),
        content_preview: String::new(),
        content_body: String::new(),
        raw_source: String::new(),
        provenance_raw: String::new(),
        metadata: Vec::new(),
        providers: std::collections::BTreeMap::new(),
        git_log: Vec::new(),
        adoption: None,
        sidecar_warning: String::new(),
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
        source_path: String::new(),
        vcs: None,
    }
}

fn make_module(name: &str, artifacts: Vec<ArtifactView>) -> ModuleView {
    ModuleView {
        name: name.to_string(),
        version: String::new(),
        description: String::new(),
        source_uri: format!("https://example.com/{name}"),
        is_target: false,
        artifacts,
        local_path: None,
        vcs: None,
        git_log: Vec::new(),
    }
}

fn sample_view() -> DashboardView {
    DashboardView {
        modules: vec![
            make_module(
                "rune-core",
                vec![make_artifact("skills", "LearnFrom", "rune-core")],
            ),
            make_module(
                "proton-agents",
                vec![make_artifact("skills", "LearnFrom", "proton-agents")],
            ),
        ],
        summary: rune::view::StatusSummary::default(),
        provenance: Vec::new(),
        adrs: Vec::new(),
        deck: None,
    }
}

#[test]
fn locate_artifact_qualified_returns_named_module() {
    let view = sample_view();
    let (located_module, located_artifact) =
        locate_artifact(&view, Some("proton-agents"), "skills", "LearnFrom").unwrap();
    assert_eq!(located_module.name, "proton-agents");
    assert_eq!(located_artifact.module, "proton-agents");
}

#[test]
fn locate_artifact_unqualified_returns_first_match() {
    let view = sample_view();
    let (located_module, _) = locate_artifact(&view, None, "skills", "LearnFrom").unwrap();
    assert_eq!(located_module.name, "rune-core");
}

#[test]
fn locate_artifact_none_for_unknown() {
    let view = sample_view();
    assert!(locate_artifact(&view, None, "skills", "Missing").is_none());
}
