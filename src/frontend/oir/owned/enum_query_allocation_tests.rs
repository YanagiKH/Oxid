//! Independent C2b query heap lifetimes through the existing global observer.
//! Candidate sources, frozen indexes, meters and caller buffers predate tracking.
//! These are pure-query controls, not evidence of source execution admission.
use super::source::reviewer_source::{integration_enabled, integration_measured};
use crate::frontend::{
    ast,
    declaration_index::{
        collect_enum_candidate, DeclarationIndex, IndexLimits, NominalExposure, NominalId,
        QualifiedValueEndpoint, SourceOwner, TypeContext, WorkMeter,
    },
    diagnostic::Diagnostic,
    hir::DefId,
    oir::owned_types::{AggregateTy, EnumId, ValueTy, VariantId},
    project::{
        budget::Allocator, ItemPathRef, ModuleId, ProjectLimits, ProjectSources, QualifiedPathRef,
    },
    source::{SourceFileId, Span},
};
use std::{
    fmt::{self, Write},
    fs,
    mem::size_of,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const SUCCESS: &str = "enum E{V} pub fn E(e:E)->E{E::V;crate::E();crate::E::V;return e;}";
const MISSING: &str = "enum E{V} fn E()->(){E::Missing;return;}";
const ROUNDS: usize = 4;
// Two original rows: the value-lane E precedes the type-lane E in lookup order.
// Classification 11+9+11, variant wrappers 11+11, callees 8+12,
// nominal/value 6+11, exposure 1, and two prepared names 9+9.
const SUCCESS_WORK: u64 = 109;
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(text: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-c2b-query-lifetimes-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("main.ox"), text).unwrap();
        Self(root)
    }

    fn load(&self) -> ProjectSources {
        ProjectSources::load_enum_index_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn quiet(work: &WorkMeter) {
    assert!(!work.observing());
    assert!(work.events.borrow().is_empty());
    assert!(work.observations.borrow().is_empty());
    assert_eq!(work.events.borrow().capacity(), 0);
    assert_eq!(work.observations.borrow().capacity(), 0);
}

fn freeze<'s>(project: &'s ProjectSources, allocator: &mut Allocator) -> DeclarationIndex<'s> {
    let build_work = WorkMeter::default();
    let index = collect_enum_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        &build_work,
        allocator,
    )
    .unwrap()
    .finish(&build_work, allocator)
    .unwrap();
    quiet(&build_work);
    assert_eq!(
        (
            index.enum_count(),
            index.function_count(),
            index.record_count()
        ),
        (1, 1, 0)
    );
    assert_eq!(index.enum_variant_counts().collect::<Vec<_>>(), [1]);
    index
}

fn paths<const N: usize>(project: &ProjectSources) -> [QualifiedPathRef; N] {
    let ast = project.try_file_ast(SourceFileId(0)).unwrap();
    let mut paths = ast
        .expressions
        .iter()
        .filter_map(|expression| match expression.kind {
            ast::ExprKind::QualifiedValue { path, .. } => Some(QualifiedPathRef {
                file: SourceFileId(0),
                path,
            }),
            _ => None,
        });
    let result = std::array::from_fn(|position| {
        let path = paths.next().unwrap();
        assert_eq!(path.path, ast::PathId(position));
        path
    });
    assert!(paths.next().is_none());
    result
}

type Trace = [(&'static str, usize, usize, bool); 16];
fn trace(allocator: &Allocator) -> Trace {
    assert_eq!(allocator.attempts, 16);
    assert_eq!(allocator.trace.len(), 16);
    assert!(!allocator.observer_trace_overflow);
    std::array::from_fn(|position| {
        let event = &allocator.trace[position];
        (event.kind, event.length, event.element_bytes, event.success)
    })
}

fn span(start: usize, end: usize) -> Span {
    Span {
        file: SourceFileId(0),
        start,
        end,
    }
}

struct Handles {
    paths: [QualifiedPathRef; 3],
    value: ast::TypeSyntax,
    nominal: ItemPathRef,
    function: ItemPathRef,
}

// Stack-only caller storage. Formatting cannot disguise an allocation in a
// String created or grown outside the global observer's view.
struct FixedText {
    bytes: [u8; 32],
    length: usize,
}
impl FixedText {
    fn new() -> Self {
        Self {
            bytes: [0; 32],
            length: 0,
        }
    }
    fn clear(&mut self) {
        self.length = 0;
    }
    fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length]).unwrap()
    }
}
impl fmt::Write for FixedText {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

fn successful_batch(
    index: &DeclarationIndex<'_>,
    handles: &Handles,
    work: &WorkMeter,
    output: &mut FixedText,
) {
    let before = work.used();
    let variant = VariantId {
        enumeration: EnumId(0),
        index: 0,
    };
    let mut query = index.query(work);
    for (path, expected) in [
        (handles.paths[0], QualifiedValueEndpoint::Variant(variant)),
        (handles.paths[1], QualifiedValueEndpoint::Function(DefId(0))),
        (handles.paths[2], QualifiedValueEndpoint::Variant(variant)),
    ] {
        assert_eq!(
            query.qualified_value_endpoint(ModuleId(0), path).unwrap(),
            expected
        );
    }
    assert_eq!(work.used() - before, 31);
    for path in [handles.paths[0], handles.paths[2]] {
        assert_eq!(query.variant(ModuleId(0), path).unwrap(), variant);
    }
    assert_eq!(work.used() - before, 53);
    for path in [
        handles.function,
        ItemPathRef {
            file: SourceFileId(0),
            path: ast::ItemPath::Absolute(handles.paths[1].path),
        },
    ] {
        assert_eq!(query.callee(ModuleId(0), path, false).unwrap(), DefId(0));
    }
    assert_eq!(work.used() - before, 73);
    assert_eq!(
        query
            .nominal_type(ModuleId(0), handles.nominal, TypeContext::Value)
            .unwrap(),
        NominalId::Enum(EnumId(0))
    );
    assert_eq!(work.used() - before, 79);
    assert_eq!(
        query
            .value_type(ModuleId(0), handles.value, TypeContext::Value)
            .unwrap(),
        ValueTy::Owned(AggregateTy::Enum(EnumId(0)))
    );
    assert_eq!(work.used() - before, 90);
    match query
        .nominal_signature_exposure(DefId(0), NominalId::Enum(EnumId(0)), handles.value.span)
        .unwrap()
    {
        NominalExposure::Denied {
            declaration,
            restrictor,
        } => {
            assert_eq!(declaration, span(5, 6));
            assert_eq!(restrictor, None);
        }
        NominalExposure::Allowed => panic!("public function cannot expose root-private E"),
    }
    assert_eq!(work.used() - before, 91);
    let first = query
        .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), handles.value.span)
        .unwrap();
    let second = query
        .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), handles.value.span)
        .unwrap();
    assert_eq!(work.used() - before, SUCCESS_WORK);
    for _ in 0..8 {
        output.clear();
        write!(output, "{first}/{second}").unwrap();
        assert_eq!(output.text(), "crate::E/crate::E");
        assert_eq!(work.used() - before, SUCCESS_WORK);
    }
}

#[test]
fn enum_query_lifecycle_repeated_successes_and_prepared_names_allocate_nothing() {
    let fixture = Fixture::new(SUCCESS);
    let project = fixture.load();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(16).unwrap();
    let index = freeze(&project, &mut allocator);
    let original_trace = trace(&allocator);
    let trace_capacity = allocator.trace.capacity();
    let ast = project.try_file_ast(SourceFileId(0)).unwrap();
    let value = ast.functions[0].params[0].ty;
    let ast::TypeSyntaxKind::Name(nominal) = value.kind else {
        panic!("genuine nominal source annotation")
    };
    assert_eq!(value.span, span(21, 22));
    assert_eq!(ast.functions[0].name, span(17, 18));
    let handles = Handles {
        paths: paths(&project),
        value,
        nominal: ItemPathRef {
            file: SourceFileId(0),
            path: nominal,
        },
        function: ItemPathRef {
            file: SourceFileId(0),
            path: ast::ItemPath::Unqualified(ast.functions[0].name),
        },
    };
    let fresh: [WorkMeter; ROUNDS] = std::array::from_fn(|_| WorkMeter::new(SUCCESS_WORK));
    let shared = WorkMeter::new(SUCCESS_WORK * ROUNDS as u64);
    let mut output = FixedText::new();

    // The first query is cold. Subsequent fresh/shared-meter rounds must all
    // perform the same literal work and make no heap call, including temporary
    // allocation followed by free. Every fresh meter is exhausted before Display.
    for work in &fresh {
        quiet(work);
        let ((), stats) =
            integration_measured(|| successful_batch(&index, &handles, work, &mut output));
        assert_eq!(stats, (0, 0, 0));
        assert_eq!(work.used(), work.limit());
        quiet(work);
        assert!(!integration_enabled());
    }
    for round in 1..=ROUNDS {
        let ((), stats) =
            integration_measured(|| successful_batch(&index, &handles, &shared, &mut output));
        assert_eq!(stats, (0, 0, 0));
        assert_eq!(shared.used(), round as u64 * SUCCESS_WORK);
        quiet(&shared);
        assert!(!integration_enabled());
    }
    assert_eq!(shared.used(), shared.limit());
    assert_eq!(trace(&allocator), original_trace);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert_eq!(project.try_text(value.span), Some("E"));
    println!("enum query success: 4 fresh + 4 shared cold/repeated batches; 109 work each; real calls/live/peak=0/0/0, fixed-buffer prepared names unchanged after exhaustion");
}

#[test]
fn enum_query_lifecycle_missing_variant_retains_only_diagnostic_then_zero() {
    let fixture = Fixture::new(MISSING);
    let project = fixture.load();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(16).unwrap();
    let index = freeze(&project, &mut allocator);
    let original_trace = trace(&allocator);
    let trace_capacity = allocator.trace.capacity();
    let [path] = paths(&project);
    // Entry/prefix/select/access/member = 8, one unequal V probe = 3,
    // bounded diagnostic name bytes = len("Missing") + len("E") = 8.
    let work = WorkMeter::new(19);
    let (result, (calls, live, peak)) = integration_measured(|| {
        index
            .query(&work)
            .qualified_value_endpoint(ModuleId(0), path)
    });
    let error = result.unwrap_err();
    assert_eq!((error.code, error.stage), ("E0200", "resolve"));
    assert_eq!(error.message, "unknown variant `Missing` of enum `E`");
    assert_eq!(error.primary, Some(span(24, 31)));
    assert_eq!(project.try_text(error.primary.unwrap()), Some("Missing"));
    assert_eq!(error.secondary.capacity(), 0);
    assert_eq!(error.notes.capacity(), 0);
    assert!(error.message.len() <= 1024);
    // Observe actual retained capacities independently of the producer's
    // reservation plan. This error owns only its Box and bounded message.
    let diagnostic_heap = size_of::<Diagnostic>() + error.message.capacity();
    assert_eq!(calls, 2);
    assert_eq!(live, isize::try_from(diagnostic_heap).unwrap());
    assert_eq!(peak, live);
    assert_eq!(work.used(), 19);
    quiet(&work);
    let ((), (drop_calls, drop_live, drop_peak)) = integration_measured(|| drop(error));
    assert_eq!((drop_calls, drop_live, drop_peak), (0, -live, 0));
    assert_eq!(live + drop_live, 0);

    // One complete ownership interval independently proves final zero, rather
    // than relying only on the sum of the two signed snapshots above.
    let work = WorkMeter::new(19);
    let ((), (repeat_calls, final_live, repeat_peak)) = integration_measured(|| {
        drop(
            index
                .query(&work)
                .qualified_value_endpoint(ModuleId(0), path)
                .unwrap_err(),
        );
    });
    assert_eq!((repeat_calls, final_live, repeat_peak), (2, 0, live));
    assert_eq!(work.used(), 19);
    quiet(&work);
    assert_eq!(trace(&allocator), original_trace);
    assert_eq!(allocator.trace.capacity(), trace_capacity);
    assert_eq!(project.try_text(span(5, 6)), Some("E"));
    assert!(!integration_enabled());
    println!("enum query missing variant only: real calls={calls}, diagnostic-only live={live}, peak={peak}, after error drop=0; other error families are outside this control");
}
