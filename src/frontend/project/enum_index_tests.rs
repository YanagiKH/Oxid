//! Real multi-file C2a parser input; no declaration or executable witness shortcuts.
use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-enum-index-loader-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), "mod a;enum E{V}").unwrap();
        fs::write(root.join("a.ox"), "enum F{W}").unwrap();
        Self(root)
    }
    #[allow(clippy::result_large_err)] // Match the existing loader's owned failure evidence.
    fn load(
        &self,
        limits: ProjectLimits,
        allocator: &mut Allocator,
    ) -> Result<ProjectSources, LoadFailure> {
        ProjectSources::load_enum_index_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            limits,
            allocator,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn exact() -> ProjectLimits {
    ProjectLimits {
        source_bytes: 24,
        tokens: 16,
        nodes: 5,
        modules: 2,
        ..ProjectLimits::default()
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_loader_preserves_global_limits_and_real_file_association() {
    let fixture = Fixture::new();
    let project = fixture.load(exact(), &mut Allocator::default()).unwrap();
    let usage = project.usage();
    assert_eq!(
        (
            usage.source_bytes,
            usage.non_eof_tokens,
            usage.syntax_nodes,
            usage.modules
        ),
        (24, 16, 5, 2)
    );
    assert_eq!(
        project.enum_handles().collect::<Vec<_>>(),
        [
            EnumAstKey {
                file: SourceFileId(0),
                index: 0
            },
            EnumAstKey {
                file: SourceFileId(1),
                index: 0
            }
        ]
    );
    for id in 0..2 {
        let key = EnumAstKey {
            file: SourceFileId(id),
            index: 0,
        };
        assert_eq!(project.try_enum(key).unwrap().name.file, key.file);
        assert!(project
            .try_file_ast(key.file)
            .unwrap()
            .belongs_to(project.sources().get(key.file)));
    }
    assert_eq!(project.modules()[1].parent, Some(ModuleId(0)));
    assert_eq!(project.modules()[1].depth, 1);
    let inventory = project.inventory().unwrap();
    assert_eq!(inventory.ast_headers, 2 * size_of::<ast::Program>());
    assert_eq!(inventory.tokens, 18 * size_of::<lexer::Token>());
    for (kind, limits) in [
        (
            "bytes",
            ProjectLimits {
                source_bytes: 23,
                ..exact()
            },
        ),
        (
            "tokens",
            ProjectLimits {
                tokens: 15,
                ..exact()
            },
        ),
        (
            "nodes",
            ProjectLimits {
                nodes: 4,
                ..exact()
            },
        ),
        (
            "modules",
            ProjectLimits {
                modules: 1,
                ..exact()
            },
        ),
    ] {
        let failure = fixture.load(limits, &mut Allocator::default()).unwrap_err();
        assert!(!failure.diagnostics.is_empty(), "{kind}");
        if kind == "nodes" {
            let at = failure.diagnostics[0].primary.unwrap();
            assert_eq!(at.file, SourceFileId(1));
            assert_eq!(failure.sources.get(at.file).text_at(at), "W");
        }
    }
    assert!(fixture
        .load(
            ProjectLimits {
                source_bytes: 25,
                tokens: 17,
                nodes: 6,
                modules: 3,
                ..exact()
            },
            &mut Allocator::default()
        )
        .is_ok());
    assert!(ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default()
    )
    .is_ok());
}

#[cfg(target_os = "linux")]
#[test]
fn enum_index_loader_all_observed_requests_fail_without_later_source_growth() {
    let fixture = Fixture::new();
    let mut allocator = Allocator::default();
    let project = fixture.load(exact(), &mut allocator).unwrap();
    let attempts = allocator.attempts;
    assert_eq!(allocator.trace.len(), attempts);
    assert!(
        allocator
            .trace
            .iter()
            .filter(|event| event.kind == "enum declarations")
            .count()
            == 2
    );
    drop(project);
    for fail_at in 1..=attempts {
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let failure = fixture.load(exact(), &mut allocator).unwrap_err();
        assert_eq!(failure.allocator.attempts, fail_at);
        assert_eq!(failure.allocator.trace.len(), fail_at);
        assert!(!failure.allocator.trace.last().unwrap().success);
        // LoadFailure deliberately retains the source/diagnostic evidence until drop.
        drop(failure);
    }
    println!("enum-index-loader allocation-requests={attempts}");
}

#[test]
fn enum_index_loader_and_project_headers_are_measured() {
    println!("enum-index-loader-layout SourceSetBuilder={} ProjectSources={} ModuleHeader={} LoadFailure={} EnumPolicy={} Frame={}",size_of::<SourceSetBuilder<'_>>(),size_of::<ProjectSources>(),size_of::<ModuleHeader>(),size_of::<LoadFailure>(),size_of::<ProjectEnumSyntax>(),size_of::<Frame>());
}

// Child modules intentionally remain unavailable until this host's filesystem
// policy is qualified. Both enum loader entry points must retain that boundary.
#[cfg(not(target_os = "linux"))]
#[test]
fn enum_index_loader_unqualified_host_stops_before_child_io() {
    let fixture = Fixture::new();
    // If a loader enters the child, it would report a parse error instead.
    fs::write(fixture.0.join("a.ox"), "@ child must not be parsed").unwrap();
    for failure in [
        fixture
            .load(exact(), &mut Allocator::default())
            .unwrap_err(),
        ProjectSources::load_typed(fixture.0.join("main.ox").to_str().unwrap(), exact())
            .unwrap_err(),
    ] {
        assert_eq!(failure.diagnostics.len(), 1);
        let error = &failure.diagnostics[0];
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            (
                "E0005",
                "source",
                "module source policy is not qualified on this host"
            )
        );
        let origin = error.primary.unwrap();
        assert_eq!(
            origin,
            Span {
                file: SourceFileId(0),
                start: 4,
                end: 5
            }
        );
        assert_eq!(failure.sources.text(origin), "a");
        assert!(error.secondary.is_empty() && error.notes.is_empty());
        assert_eq!(failure.sources.files().len(), 1);
        assert_eq!(
            failure.sources.get(SourceFileId(0)).text(),
            "mod a;enum E{V}"
        );
        assert_eq!(
            (
                failure.usage.source_bytes,
                failure.usage.non_eof_tokens,
                failure.usage.syntax_nodes,
                failure.usage.modules
            ),
            (15, 10, 3, 1)
        );
        assert_eq!(
            (
                failure.usage.probes,
                failure.usage.directory_entries,
                failure.usage.directory_name_units
            ),
            (0, 0, 0)
        );
        assert!(!failure
            .allocator
            .trace
            .iter()
            .any(|event| event.kind == "module probe path"));
        assert!(failure.allocator.trace.iter().all(|event| event.success));
        assert_eq!(failure.allocator.trace.len(), failure.allocator.attempts);
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn enum_index_loader_unqualified_host_reserve_failures_stop_at_the_paid_prefix() {
    let fixture = Fixture::new();
    let baseline = fixture
        .load(exact(), &mut Allocator::default())
        .unwrap_err();
    assert_eq!(baseline.diagnostics[0].code, "E0005");
    assert!(baseline.allocator.attempts > 0);
    for fail_at in 1..=baseline.allocator.attempts {
        let mut allocator = Allocator {
            fail_at: Some(fail_at),
            ..Allocator::default()
        };
        let failure = fixture.load(exact(), &mut allocator).unwrap_err();
        assert_eq!(failure.diagnostics[0].code, "E0400");
        assert!(matches!(
            failure.diagnostics[0].stage,
            "source-project" | "parse"
        ));
        assert_eq!(failure.allocator.attempts, fail_at);
        assert_eq!(failure.allocator.trace.len(), fail_at);
        assert!(failure.sources.files().len() <= 1);
        assert_eq!(
            (failure.usage.probes, failure.usage.directory_entries),
            (0, 0)
        );
        for (position, (actual, expected)) in failure
            .allocator
            .trace
            .iter()
            .zip(&baseline.allocator.trace)
            .enumerate()
        {
            assert_eq!(
                (
                    actual.kind,
                    actual.length,
                    actual.element_bytes,
                    actual.success
                ),
                (
                    expected.kind,
                    expected.length,
                    expected.element_bytes,
                    position + 1 != fail_at
                )
            );
        }
    }
}
