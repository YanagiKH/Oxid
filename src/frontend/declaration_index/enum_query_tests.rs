//! C2b pure index queries. Successful queries do not open source execution.
//! Multi-file positive queries follow the production Linux filesystem policy;
//! local queries, display carriers, and entry-file parse limits are portable.
use super::*;
use crate::frontend::project::ProjectLimits;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "oxid-c2b-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for (name, text) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
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
fn freeze(project: &ProjectSources) -> DeclarationIndex<'_> {
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    collect_enum_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        &work,
        &mut allocator,
    )
    .unwrap()
    .finish(&work, &mut allocator)
    .unwrap()
}
fn paths(project: &ProjectSources, file: usize) -> Vec<QualifiedPathRef> {
    project
        .try_file_ast(SourceFileId(file))
        .unwrap()
        .expressions
        .iter()
        .filter_map(|expression| match expression.kind {
            ast::ExprKind::QualifiedValue { path, .. } => Some(QualifiedPathRef {
                file: SourceFileId(file),
                path,
            }),
            _ => None,
        })
        .collect()
}

#[test]
fn enum_query_literal_classification_work_and_legacy_callee_costs() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "enum E{V} fn E()->(){E::V;crate::E();crate::E::V;return;}",
    )]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    assert_eq!(paths.len(), 3);
    let variant = VariantId {
        enumeration: EnumId(0),
        index: 0,
    };
    for (position, (expected, cost, range, terminal)) in [
        (
            QualifiedValueEndpoint::Variant(variant),
            11,
            (21, 25),
            (24, 25),
        ),
        (
            QualifiedValueEndpoint::Function(DefId(0)),
            9,
            (26, 34),
            (33, 34),
        ),
        (
            QualifiedValueEndpoint::Variant(variant),
            11,
            (37, 48),
            (47, 48),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(paths[position].path, ast::PathId(position));
        let work = WorkMeter::new(cost);
        work.enable_observation();
        assert_eq!(
            index
                .query(&work)
                .qualified_value_endpoint(ModuleId(0), paths[position])
                .unwrap(),
            expected
        );
        assert_eq!(work.used(), cost);
        let origin = Span {
            file: SourceFileId(0),
            start: range.0,
            end: range.1,
        };
        if let QualifiedValueEndpoint::Variant(expected) = expected {
            assert!(work.observations.borrow().iter().any(|event|matches!(event,Observation::VariantTarget {origin:at,variant,..} if *at==origin && *variant==expected)));
            assert!(!work
                .observations
                .borrow()
                .iter()
                .any(|event| matches!(event, Observation::Target { .. })));
        }
        let denied = WorkMeter::new(cost - 1);
        denied.enable_observation();
        let error = index
            .query(&denied)
            .qualified_value_endpoint(ModuleId(0), paths[position])
            .unwrap_err();
        assert_eq!(
            (error.code, error.primary),
            (
                "E0400",
                Some(Span {
                    file: SourceFileId(0),
                    start: terminal.0,
                    end: terminal.1
                })
            )
        );
        assert_eq!(denied.used(), cost - 1);
        assert!(!denied.observations.borrow().iter().any(|event| matches!(
            event,
            Observation::Target { .. } | Observation::VariantTarget { .. }
        )));
    }
    let name = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].name;
    for (path, cost) in [
        (ast::ItemPath::Unqualified(name), 8),
        (ast::ItemPath::Absolute(paths[1].path), 12),
    ] {
        let work = WorkMeter::new(cost);
        assert_eq!(
            index
                .query(&work)
                .callee(
                    ModuleId(0),
                    ItemPathRef {
                        file: SourceFileId(0),
                        path
                    },
                    false
                )
                .unwrap(),
            DefId(0)
        );
        assert_eq!(work.used(), cost);
    }
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .variant(ModuleId(0), paths[0])
            .unwrap(),
        variant
    );
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .variant(ModuleId(0), paths[1])
            .unwrap_err()
            .code,
        "E0202"
    );
    assert_eq!(
        index.require_current_source_pipeline().unwrap_err().code,
        "E0101"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_imported_alias_value_parameter_and_record_only_reference() {
    let fixture=Fixture::new(&[
        ("main.ox","mod m; use crate::m::E as A; fn main()->(){A::V;crate::m::E::V;crate::m::f();return;} fn take(x:A)->A{return x;} fn borrow(x:&A)->(){return;}"),
        ("m.ox","pub enum E{V} pub fn f()->(){return;}"),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    let work = WorkMeter::default();
    for path in &paths[..2] {
        assert_eq!(
            index.query(&work).variant(ModuleId(0), *path).unwrap(),
            VariantId {
                enumeration: EnumId(0),
                index: 0
            }
        );
    }
    assert_eq!(
        index
            .query(&work)
            .qualified_value_endpoint(ModuleId(0), paths[2])
            .unwrap(),
        QualifiedValueEndpoint::Function(DefId(3))
    );
    let ast = project.try_file_ast(SourceFileId(0)).unwrap();
    let value = ast.functions[1].params[0].ty;
    assert_eq!(
        index
            .query(&work)
            .value_type(ModuleId(0), value, TypeContext::Value)
            .unwrap(),
        ValueTy::Owned(AggregateTy::Enum(EnumId(0)))
    );
    assert_eq!(
        index
            .query(&work)
            .parameter_type(ModuleId(0), value)
            .unwrap(),
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(EnumId(0))))
    );
    let ast::TypeSyntaxKind::Name(path) = value.kind else {
        panic!("nominal source parameter")
    };
    assert_eq!(
        index
            .query(&work)
            .nominal_type(
                ModuleId(0),
                ItemPathRef {
                    file: SourceFileId(0),
                    path
                },
                TypeContext::Value
            )
            .unwrap(),
        NominalId::Enum(EnumId(0))
    );
    assert_eq!(
        index
            .query(&work)
            .record_type(
                ModuleId(0),
                ItemPathRef {
                    file: SourceFileId(0),
                    path
                },
                TypeContext::Constructor
            )
            .unwrap_err()
            .code,
        "E0202"
    );
    assert_eq!(
        index
            .query(&work)
            .parameter_type(ModuleId(0), ast.functions[2].params[0].ty)
            .unwrap_err()
            .code,
        "E0202"
    );
    assert_eq!(
        index.require_current_source_pipeline().unwrap_err().code,
        "E0101"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_private_prefix_and_function_errors_never_fall_back() {
    let fixture = Fixture::new(&[
        (
            "main.ox",
            "mod m;fn main()->(){crate::m::E::Missing;crate::m::f();crate::m::E();return;}",
        ),
        (
            "m.ox",
            "enum E{V} fn f()->(){return;} pub fn E()->(){return;}",
        ),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    for (position, name) in [(0, "E"), (1, "f")] {
        let work = WorkMeter::default();
        work.enable_observation();
        let error = index
            .query(&work)
            .qualified_value_endpoint(ModuleId(0), paths[position])
            .unwrap_err();
        assert_eq!(error.code, "E0206");
        assert_eq!(project.text(error.primary.unwrap()), name);
        assert!(!work
            .events
            .borrow()
            .iter()
            .any(|event| event.operation == "variant lookup probe"));
        assert!(!work.observations.borrow().iter().any(|event| matches!(
            event,
            Observation::Target { .. } | Observation::VariantTarget { .. }
        )));
    }
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .qualified_value_endpoint(ModuleId(0), paths[2])
            .unwrap(),
        QualifiedValueEndpoint::Function(DefId(2))
    );
}

#[test]
fn enum_query_actual_fixed_and_prepared_name_carriers_are_measured() {
    use std::mem::align_of;
    println!("enum-query-layout Prefix={}/{} Endpoint={}/{} NominalExposure={}/{} LegacyExposure={}/{} NominalId={}/{} Prepared={}/{} OptionalPrepared={}/{} Fixed={}",size_of::<AbsolutePrefix>(),align_of::<AbsolutePrefix>(),size_of::<QualifiedValueEndpoint>(),align_of::<QualifiedValueEndpoint>(),size_of::<NominalExposure>(),align_of::<NominalExposure>(),size_of::<Exposure>(),align_of::<Exposure>(),size_of::<NominalId>(),align_of::<NominalId>(),size_of::<PreparedTypeName<'_>>(),align_of::<PreparedTypeName<'_>>(),size_of::<Option<PreparedTypeName<'_>>>(),align_of::<Option<PreparedTypeName<'_>>>(),FIXED_SCRATCH);
    const { assert!(FIXED_SCRATCH > 3968) };
    const { assert!(FIXED_SCRATCH <= 4096) };
    #[cfg(target_pointer_width = "64")]
    assert_eq!(size_of::<PreparedTypeName<'_>>(), 464);
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_exposure_matches_literal_subtree_inclusion_matrix() {
    let fixture=Fixture::new(&[
        ("main.ox","pub mod a; pub mod b; pub enum All{V} enum Root{V} pub fn external(x:All)->(){return;} fn root_fn(x:Root)->(){return;}"),
        ("a.ox","pub mod c; enum A{V} pub enum PublicA{V} fn a_fn(x:A)->(){return;} pub fn a_public(x:A)->(){return;}"),
        ("a/c.ox","enum C{V} fn c_fn(x:C)->(){return;}"),
        ("b.ox","enum B{V} fn b_fn(x:B)->(){return;}"),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    // Nominal domains ALL/root/a/c/b versus function domains ALL/root/a/c/b.
    let enum_ids = [0, 1, 2, 4, 5];
    let functions = [0, 1, 2, 4, 5];
    let expected = [
        [true, true, true, true, true],
        [false, true, true, true, true],
        [false, false, true, true, false],
        [false, false, false, true, false],
        [false, false, false, false, true],
    ];
    for (row, enumeration) in enum_ids.into_iter().enumerate() {
        for (column, function) in functions.into_iter().enumerate() {
            let (key, module) = index.function(DefId(function)).unwrap();
            let at = index.sources().ast(module).unwrap().functions[key.index].params[0]
                .ty
                .span;
            let work = WorkMeter::new(1);
            let result = index
                .query(&work)
                .nominal_signature_exposure(
                    DefId(function),
                    NominalId::Enum(EnumId(enumeration)),
                    at,
                )
                .unwrap();
            assert_eq!(
                matches!(result, NominalExposure::Allowed),
                expected[row][column]
            );
            assert_eq!(work.used(), 1);
            if let NominalExposure::Denied {
                declaration,
                restrictor,
            } = result
            {
                assert_eq!(
                    declaration,
                    index.enum_view(EnumId(enumeration)).unwrap().name_span()
                );
                assert_eq!(restrictor, None);
            }
        }
    }
    // Public a_public can name A locally but exposes it to a wider domain.
    let function = &project.try_file_ast(SourceFileId(1)).unwrap().functions[1];
    assert!(matches!(
        index
            .query(&WorkMeter::default())
            .nominal_signature_exposure(
                DefId(3),
                NominalId::Enum(EnumId(2)),
                function.params[0].ty.span
            )
            .unwrap(),
        NominalExposure::Denied { .. }
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_ancestor_exposure_and_legacy_record_wrapper_agree() {
    let fixture=Fixture::new(&[
        ("main.ox","mod hidden; pub fn api(x:crate::hidden::E)->(){return;} pub fn record_api(x:crate::hidden::R)->(){return;}"),
        ("hidden.ox","pub enum E{V} pub struct R{} pub fn inside(x:E)->(){return;}"),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    let root = project.try_file_ast(SourceFileId(0)).unwrap();
    let child = project.try_file_ast(SourceFileId(1)).unwrap();
    let at = root.functions[0].params[0].ty.span;
    let result = index
        .query(&WorkMeter::new(1))
        .nominal_signature_exposure(DefId(0), NominalId::Enum(EnumId(0)), at)
        .unwrap();
    assert!(
        matches!(result,NominalExposure::Denied {declaration,restrictor:Some(restrictor)} if declaration==child.enums[0].name && restrictor==root.modules[0].name)
    );
    assert!(matches!(
        index
            .query(&WorkMeter::new(1))
            .nominal_signature_exposure(
                DefId(2),
                NominalId::Enum(EnumId(0)),
                child.functions[0].params[0].ty.span
            )
            .unwrap(),
        NominalExposure::Allowed
    ));
    let record_at = root.functions[1].params[0].ty.span;
    let old = index
        .query(&WorkMeter::new(1))
        .signature_exposure(DefId(1), RecordId(0), record_at)
        .unwrap();
    let nominal = index
        .query(&WorkMeter::new(1))
        .nominal_signature_exposure(DefId(1), NominalId::Record(RecordId(0)), record_at)
        .unwrap();
    match (old, nominal) {
        (
            Exposure::Denied {
                record,
                restrictor: left,
            },
            NominalExposure::Denied {
                declaration,
                restrictor: right,
            },
        ) => {
            assert_eq!(record, declaration);
            assert_eq!(left, right);
            assert_eq!(left, Some(root.modules[0].name));
        }
        other => panic!("expected matching denied exposure: {other:?}"),
    }
    for (function, nominal, origin) in [
        (DefId(usize::MAX), NominalId::Enum(EnumId(0)), at),
        (DefId(0), NominalId::Enum(EnumId(usize::MAX)), at),
        (DefId(0), NominalId::Record(RecordId(usize::MAX)), at),
        (DefId(0), NominalId::Enum(EnumId(0)), child.enums[0].name),
    ] {
        assert_eq!(
            index
                .query(&WorkMeter::default())
                .nominal_signature_exposure(function, nominal, origin)
                .unwrap_err()
                .code,
            "E0500"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_bounded_names_preserve_kind_work_and_infallible_display() {
    let module = "m".repeat(200);
    let root = format!("pub mod {module};fn root()->(){{return;}}");
    let file = format!("{module}.ox");
    let fixture = Fixture::new(&[("main.ox", &root), (&file, "pub enum E{V} pub struct R{}")]);
    let project = fixture.load();
    let index = freeze(&project);
    let at = project.try_file_ast(SourceFileId(0)).unwrap().functions[0].name;
    let enum_work = WorkMeter::new(162);
    let record_work = WorkMeter::new(162);
    let enumeration = index
        .query(&enum_work)
        .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), at)
        .unwrap();
    let record = index
        .query(&record_work)
        .prepare_type_name(RecordId(0), at)
        .unwrap();
    assert_eq!((enum_work.used(), record_work.used()), (162, 162));
    let expected_enum = format!("crate::{}...::E [enum #0]", "m".repeat(137));
    let expected_record = format!("crate::{}...::R [record #0]", "m".repeat(135));
    for _ in 0..2 {
        assert_eq!(enumeration.to_string(), expected_enum);
        assert_eq!(record.to_string(), expected_record);
    }
    assert_eq!((expected_enum.len(), expected_record.len()), (160, 160));
    assert_eq!((enum_work.used(), record_work.used()), (162, 162));
    let denied = WorkMeter::new(161);
    let error = index
        .query(&denied)
        .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), at)
        .unwrap_err();
    assert_eq!(
        (error.code, error.primary, denied.used()),
        ("E0400", Some(at), 2)
    );
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .prepare_nominal_type_name(NominalId::Enum(EnumId(usize::MAX)), at)
            .unwrap_err()
            .code,
        "E0500"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_name_length_boundaries_and_utf8_display_are_independent() {
    for length in [159, 160, 161] {
        let module = "m".repeat(length - 10);
        let root = format!("pub mod {module};");
        let file = format!("{module}.ox");
        let fixture = Fixture::new(&[("main.ox", &root), (&file, "pub enum E{V}")]);
        let project = fixture.load();
        let index = freeze(&project);
        let at = project.try_file_ast(SourceFileId(0)).unwrap().modules[0].name;
        let work = WorkMeter::new((2 + length.min(160)) as u64);
        let display = index
            .query(&work)
            .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), at)
            .unwrap()
            .to_string();
        assert_eq!(display.len(), length.min(160));
        if length <= 160 {
            assert_eq!(display, format!("crate::{module}::E"));
        } else {
            assert!(display.ends_with(" [enum #0]"));
        }
    }
    for length in [63, 64, 65] {
        let module = "m".repeat(200);
        let terminal = "T".repeat(length);
        let root = format!("pub mod {module};");
        let file = format!("{module}.ox");
        let child = format!("pub enum {terminal}{{V}}");
        let fixture = Fixture::new(&[("main.ox", &root), (&file, &child)]);
        let project = fixture.load();
        let index = freeze(&project);
        let at = project.try_file_ast(SourceFileId(0)).unwrap().modules[0].name;
        let display = index
            .query(&WorkMeter::default())
            .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), at)
            .unwrap()
            .to_string();
        let shown = if length <= 64 {
            terminal
        } else {
            format!("{}...", "T".repeat(61))
        };
        assert_eq!(display.len(), 160);
        assert!(display.ends_with(&format!("::{shown} [enum #0]")));
    }
}

#[test]
fn enum_query_utf8_display_preserves_boundaries_without_source_identifiers() {
    // Direct formatter data, deliberately not a claim of Unicode source identifiers.
    let module = "é".repeat(100);
    let terminal = "λ".repeat(40);
    let mut map = crate::frontend::source::SourceMap::new();
    map.add("formatter.ox".into(), format!("//{module}\n"));
    let file = map.get(SourceFileId(0));
    let ast =
        crate::frontend::parser::parse(file, crate::frontend::lexer::lex(file).unwrap()).unwrap();
    let owner = SourceOwner::original(
        file,
        &ast,
        crate::frontend::source::SourceView::Single(file),
    )
    .unwrap();
    let mut names = [CompactSpan::default(); 32];
    names[0] = CompactSpan::new(file.span(2, 2 + module.len())).unwrap();
    let prepared = PreparedTypeName {
        names,
        sources: owner,
        count: 1,
        terminal: &terminal,
        ordinal: 0,
        total: 7 + module.len() + 2 + terminal.len(),
        original: false,
        is_enum: true,
        builtin: false,
    };
    let display = prepared.to_string();
    assert!(display.len() <= 160);
    assert!(display.ends_with(" [enum #0]"));
    assert!(std::str::from_utf8(display.as_bytes()).is_ok());
}

fn deep_fixture(depth: usize, variant: bool, extra: bool) -> Fixture {
    let mut path = String::from("crate");
    for level in 0..depth {
        path.push_str(&format!("::m{level}"));
    }
    path.push_str(if variant { "::E::V" } else { "::f" });
    if extra {
        path.push_str("::Tail");
    }
    let statement = if variant {
        format!("{path};")
    } else {
        format!("{path}();")
    };
    let mut files = vec![(
        String::from("main.ox"),
        format!("pub mod m0;fn root()->(){{{statement}return;}}"),
    )];
    for level in 0..depth {
        let prefix = (0..level)
            .map(|i| format!("m{i}"))
            .collect::<Vec<_>>()
            .join("/");
        let file = if prefix.is_empty() {
            format!("m{level}.ox")
        } else {
            format!("{prefix}/m{level}.ox")
        };
        let text = if level + 1 < depth {
            format!("pub mod m{};", level + 1)
        } else {
            String::from("pub enum E{V} pub fn f()->(){return;}")
        };
        files.push((file, text));
    }
    let borrowed: Vec<_> = files
        .iter()
        .map(|(file, text)| (file.as_str(), text.as_str()))
        .collect();
    Fixture::new(&borrowed)
}
#[cfg(target_os = "linux")]
#[test]
fn enum_query_full_path_limit_counts_member_and_function_endpoints() {
    for (depth, variant, expected) in [
        (
            31,
            true,
            QualifiedValueEndpoint::Variant(VariantId {
                enumeration: EnumId(0),
                index: 0,
            }),
        ),
        (32, false, QualifiedValueEndpoint::Function(DefId(1))),
    ] {
        let fixture = deep_fixture(depth, variant, false);
        let project = fixture.load();
        let index = freeze(&project);
        let path = paths(&project, 0)[0];
        assert_eq!(
            index
                .sources()
                .qualified_path(path)
                .unwrap()
                .segments()
                .len(),
            34
        );
        assert_eq!(
            index
                .query(&WorkMeter::default())
                .qualified_value_endpoint(ModuleId(0), path)
                .unwrap(),
            expected
        );
        let last = project.try_file_ast(SourceFileId(depth)).unwrap().enums[0].name;
        let prepared = index
            .query(&WorkMeter::default())
            .prepare_nominal_type_name(NominalId::Enum(EnumId(0)), last)
            .unwrap();
        let mut expected = String::from("crate");
        for level in 0..depth {
            expected.push_str(&format!("::m{level}"));
        }
        expected.push_str("::E");
        assert_eq!(prepared.to_string(), expected);
    }
}

#[test]
fn enum_query_oversized_path_rejects_before_child_module_loading() {
    for (depth, variant) in [(31, true), (32, false)] {
        let oversized = deep_fixture(depth, variant, true);
        let failure = ProjectSources::load_enum_index_candidate(
            oversized.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        let error = &failure.diagnostics[0];
        assert_eq!(
            (error.code, error.stage, error.message.as_str()),
            ("E0400", "parse", "qualified path segment limit exceeded")
        );
        assert_eq!(failure.sources.text(error.primary.unwrap()), "Tail");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn enum_query_local_aliases_do_not_authorize_absolute_aliases_or_extra_members() {
    let fixture=Fixture::new(&[
        ("main.ox","pub mod a;pub mod b;use crate::a::E as A;fn f()->(){A::V;crate::a::E::V;crate::a::g();crate::b::A::V;crate::A::V;crate::a::E::V::Tail;return;}"),
        ("a.ox","pub enum E{V,U(())}pub fn g()->(){return;}"),
        ("b.ox","use crate::a::E as A;fn local()->(){A::V;return;}"),
    ]);
    let project = fixture.load();
    let index = freeze(&project);
    let root_paths = paths(&project, 0);
    for path in [root_paths[0], root_paths[1]] {
        assert_eq!(
            index
                .query(&WorkMeter::default())
                .variant(ModuleId(0), path)
                .unwrap(),
            VariantId {
                enumeration: EnumId(0),
                index: 0
            }
        );
    }
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .qualified_value_endpoint(ModuleId(0), root_paths[2])
            .unwrap(),
        QualifiedValueEndpoint::Function(DefId(1))
    );
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .variant(ModuleId(2), paths(&project, 2)[0])
            .unwrap(),
        VariantId {
            enumeration: EnumId(0),
            index: 0
        }
    );
    for (path, spelling) in [
        (root_paths[3], "A"),
        (root_paths[4], "A"),
        (root_paths[5], "E"),
    ] {
        let work = WorkMeter::default();
        work.enable_observation();
        let error = index
            .query(&work)
            .qualified_value_endpoint(ModuleId(0), path)
            .unwrap_err();
        assert_eq!(error.code, "E0205");
        assert_eq!(project.text(error.primary.unwrap()), spelling);
        assert!(!work
            .events
            .borrow()
            .iter()
            .any(|event| event.operation == "variant lookup probe"));
    }
}

#[test]
fn enum_query_missing_member_wrong_kind_and_foreign_handles_fail_closed() {
    let fixture = Fixture::new(&[(
        "main.ox",
        "enum E{V}enum F{V}struct R{}fn f()->(){E::V;F::V;E::Missing;Unknown::V;R::V;return;}",
    )]);
    let project = fixture.load();
    let index = freeze(&project);
    let paths = paths(&project, 0);
    for (position, id) in [(0, 0), (1, 1)] {
        let work = WorkMeter::default();
        work.enable_observation();
        let expected = VariantId {
            enumeration: EnumId(id),
            index: 0,
        };
        assert_eq!(
            index
                .query(&work)
                .variant(ModuleId(0), paths[position])
                .unwrap(),
            expected
        );
        assert!(work.observations.borrow().iter().any(
            |event| matches!(event,Observation::VariantTarget {variant,..} if *variant==expected)
        ));
    }
    let error = index
        .query(&WorkMeter::default())
        .qualified_value_endpoint(ModuleId(0), paths[2])
        .unwrap_err();
    assert_eq!(
        (error.code, error.stage, error.message.as_str()),
        ("E0200", "resolve", "unknown variant `Missing` of enum `E`")
    );
    assert_eq!(project.text(error.primary.unwrap()), "Missing");
    for position in [3, 4] {
        let work = WorkMeter::default();
        work.enable_observation();
        assert_eq!(
            index
                .query(&work)
                .qualified_value_endpoint(ModuleId(0), paths[position])
                .unwrap_err()
                .code,
            "E0202"
        );
        assert!(!work
            .events
            .borrow()
            .iter()
            .any(|event| event.operation == "variant lookup probe"));
    }
    for (requester, path) in [
        (ModuleId(1), paths[0]),
        (
            ModuleId(0),
            QualifiedPathRef {
                file: SourceFileId(1),
                path: paths[0].path,
            },
        ),
        (
            ModuleId(0),
            QualifiedPathRef {
                file: SourceFileId(0),
                path: ast::PathId(usize::MAX),
            },
        ),
    ] {
        let work = WorkMeter::default();
        work.enable_observation();
        assert_eq!(
            index
                .query(&work)
                .qualified_value_endpoint(requester, path)
                .unwrap_err()
                .code,
            "E0500"
        );
        assert!(!work.observations.borrow().iter().any(|event| matches!(
            event,
            Observation::VariantTarget { .. } | Observation::Target { .. }
        )));
    }
    let local = ItemPathRef {
        file: SourceFileId(0),
        path: ast::ItemPath::Absolute(paths[0].path),
    };
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .callee(ModuleId(0), local, false)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(
        index
            .query(&WorkMeter::default())
            .nominal_type(ModuleId(0), local, TypeContext::Value)
            .unwrap_err()
            .code,
        "E0500"
    );
    assert_eq!(
        index
            .enum_view(EnumId(0))
            .unwrap()
            .variant(VariantId {
                enumeration: EnumId(1),
                index: 0
            })
            .unwrap_err()
            .code,
        "E0500"
    );
}

#[test]
fn enum_query_complete_return_carriers_fit_the_explicit_fixed_ledger() {
    let old = size_of::<Result<Exposure, Box<Diagnostic>>>();
    let at = size_of::<Span>();
    assert!(old + at <= size_of::<[Span; 4]>());
    let added = size_of::<Result<AbsolutePrefix, Box<Diagnostic>>>()
        + size_of::<Result<QualifiedValueEndpoint, Box<Diagnostic>>>()
        + size_of::<Result<NominalExposure, Box<Diagnostic>>>()
        + size_of::<Result<NominalId, Box<Diagnostic>>>()
        + size_of::<&ItemPathRef>();
    println!("enum-query-return-layout PrefixResult={} EndpointResult={} NominalExposureResult={} NominalIdResult={} LegacyExposureResult={} UseSpan={} LegacyEnvelope={} Added={} Fixed={}",size_of::<Result<AbsolutePrefix,Box<Diagnostic>>>(),size_of::<Result<QualifiedValueEndpoint,Box<Diagnostic>>>(),size_of::<Result<NominalExposure,Box<Diagnostic>>>(),size_of::<Result<NominalId,Box<Diagnostic>>>(),old,at,size_of::<[Span;4]>(),added,FIXED_SCRATCH);
    // Keep the earlier literal baseline and disclose each scoped carrier delta.
    let builtin = size_of::<BuiltinAdmission>()
        + size_of::<Result<DeclarationProjection, Box<Diagnostic>>>()
        + size_of::<Result<Option<DeclarationHandle>, Box<Diagnostic>>>()
        + size_of::<Result<BuiltinItem, Box<Diagnostic>>>()
        + size_of::<Option<&ast::EnumDecl>>()
        + size_of::<Option<&ast::EnumVariantSyntax>>()
        + size_of::<source_owner::QualifiedPathView<'_>>()
        + size_of::<bool>()
        // The checked previous cursor now uses CompactSpan's u32 domain.
        + size_of::<u32>()
        + size_of::<BuiltinEnum>()
        + size_of::<BuiltinFunction>()
        + size_of::<super::super::parser::StdImportPolicy>()
        + size_of::<Result<DeclarationOrigin, Box<Diagnostic>>>()
        + size_of::<[usize; 2]>()
        + size_of::<Option<&str>>();
    let table_growth = size_of::<Tables<'_>>() - 344;
    let names_saved = 2 * (576 - size_of::<PreparedTypeName<'_>>());
    assert_eq!(
        FIXED_SCRATCH,
        3968 + added + table_growth + builtin - names_saved
    );
    println!("builtin-index-fixed-breakdown baseline=4088 table_growth={table_growth} new_carriers={builtin} name_savings={names_saved} complete={FIXED_SCRATCH}");
}

#[test]
fn enum_query_bounded_variant_scan_has_literal_costs_through_256_members() {
    for (count, last_cost, missing_cost) in
        [(1, 14, 13), (2, 20, 16), (255, 1088, 775), (256, 1094, 778)]
    {
        let variants = (0..count)
            .map(|id| format!("V{id:03}"))
            .collect::<Vec<_>>()
            .join(",");
        let last = format!("V{:03}", count - 1);
        let text = format!("enum E{{{variants}}}fn E()->(){{E::V000;E::{last};E::X;return;}}");
        let fixture = Fixture::new(&[("main.ox", &text)]);
        let project = fixture.load();
        let index = freeze(&project);
        let paths = paths(&project, 0);
        // Base8 = entry1 + prefix-segment1 + type lookup3 + permission2 + member-segment1.
        // V254 scan = 100*4 + 100*4 + 50*5 + 4*6 + 6 =1080; V255 adds one6.
        // An absent X pays3 per member, then diagnostic-name bytes X1+E1.
        assert_eq!(missing_cost, 10 + 3 * count);
        for (position, ordinal, cost) in [(0, 0, 14), (1, count - 1, last_cost)] {
            let work = WorkMeter::new(cost as u64);
            work.enable_observation();
            let expected = VariantId {
                enumeration: EnumId(0),
                index: ordinal,
            };
            assert_eq!(
                index
                    .query(&work)
                    .variant(ModuleId(0), paths[position])
                    .unwrap(),
                expected
            );
            assert_eq!(work.used(), cost as u64);
            assert_eq!(
                work.events
                    .borrow()
                    .iter()
                    .filter(|event| event.operation == "variant lookup probe")
                    .count(),
                ordinal + 1
            );
            let short = WorkMeter::new(cost as u64 - 1);
            short.enable_observation();
            let error = index
                .query(&short)
                .variant(ModuleId(0), paths[position])
                .unwrap_err();
            assert_eq!(error.code, "E0400");
            assert_eq!(short.used(), cost as u64 - 1);
            assert_eq!(
                project.text(error.primary.unwrap()),
                if position == 0 { "V000" } else { last.as_str() }
            );
            assert!(!short
                .observations
                .borrow()
                .iter()
                .any(|event| matches!(event, Observation::VariantTarget { .. })));
        }
        let work = WorkMeter::new(missing_cost as u64);
        work.enable_observation();
        let error = index
            .query(&work)
            .variant(ModuleId(0), paths[2])
            .unwrap_err();
        assert_eq!(
            (error.code, project.text(error.primary.unwrap())),
            ("E0200", "X")
        );
        assert_eq!(work.used(), missing_cost as u64);
        assert_eq!(
            work.events
                .borrow()
                .iter()
                .filter(|event| event.operation == "variant lookup probe")
                .count(),
            count
        );
        let short = WorkMeter::new(missing_cost as u64 - 1);
        short.enable_observation();
        let error = index
            .query(&short)
            .variant(ModuleId(0), paths[2])
            .unwrap_err();
        assert_eq!(
            (error.code, project.text(error.primary.unwrap())),
            ("E0400", "X")
        );
        assert_eq!(
            short.used(),
            missing_cost as u64 - 2,
            "the unpaid diagnostic debit is a two-unit batch"
        );
        assert!(!short
            .observations
            .borrow()
            .iter()
            .any(|event| matches!(event, Observation::VariantTarget { .. })));
    }
    println!("enum-query-item-handles ItemPathRef={} FourHandles={} BankHandleWords={} RemainingBankWords={} ExtraHelperReference={} Fixed={}",size_of::<ItemPathRef>(),size_of::<[ItemPathRef;4]>(),ITEM_HANDLE_WORDS,128-ITEM_HANDLE_WORDS,size_of::<&ItemPathRef>(),FIXED_SCRATCH);
}

#[cfg(target_os = "linux")]
#[test]
fn builtin_compact_prepared_name_preserves_32_module_boundary() {
    let fixture = deep_fixture(33, false, false);
    // Avoid the separate qualified-path segment gate: this tests loader depth.
    fs::write(
        fixture.0.join("main.ox"),
        "pub mod m0;fn root()->(){return;}",
    )
    .unwrap();
    let failure = ProjectSources::load_builtin_candidate(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "E0400");
    assert_eq!(
        failure.diagnostics[0].message,
        "module depth limit exceeded"
    );
}
