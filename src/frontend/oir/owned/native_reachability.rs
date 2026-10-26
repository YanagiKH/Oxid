//! Selection of emitted bodies after complete native admission.
use super::*;

pub(super) trait EmittedFunctions: Copy {
    const CONTROL_BYTES: usize;
    fn build(plan: &ExecutionPlan<'_>, entry: hir::DefId) -> Result<Self, Box<Diagnostic>>;
    fn contains(self, function: hir::DefId) -> bool;
    fn work(self) -> usize;
}

/// The inherited route has no selector storage or graph walk.
#[derive(Clone, Copy)]
pub(super) struct Full;
impl EmittedFunctions for Full {
    const CONTROL_BYTES: usize = 0;
    fn build(_: &ExecutionPlan<'_>, _: hir::DefId) -> Result<Self, Box<Diagnostic>> {
        Ok(Self)
    }
    fn contains(self, _: hir::DefId) -> bool {
        true
    }
    fn work(self) -> usize {
        0
    }
}

const MAX_SELECTION_FUNCTIONS: usize = super::super::super::native::MAX_HOSTED_FUNCTIONS;
const _: () = assert!(MAX_SELECTION_FUNCTIONS >= MAX_FUNCTIONS);
const WORDS: usize = MAX_SELECTION_FUNCTIONS.div_ceil(u64::BITS as usize);

#[derive(Clone, Copy)]
pub(super) struct Reachable {
    bits: [u64; WORDS],
    work: usize,
}

// Named fixed storage: a retained selector, the pending bitset, the returned
// selector, loop/iterator/index/target roles and caller/callee copies. This is
// a separate additive bound, never spare capacity in the inherited envelope.
const CONTROL_BYTES: usize = 1024;
const _: () = assert!(
    3 * size_of::<Reachable>() + size_of::<[u64; WORDS]>() + 40 * size_of::<usize>()
        <= CONTROL_BYTES
);

impl EmittedFunctions for Reachable {
    const CONTROL_BYTES: usize = CONTROL_BYTES;

    fn build(plan: &ExecutionPlan<'_>, entry: hir::DefId) -> Result<Self, Box<Diagnostic>> {
        let functions = plan.witness().functions();
        if functions.len() > MAX_SELECTION_FUNCTIONS || entry.0 >= functions.len() {
            return Err(mismatch("native reachability entry"));
        }
        let mut selected = Self {
            bits: [0; WORDS],
            // Also charge the additional resource-inventory selection scan.
            work: functions.len(),
        };
        let mut pending = [0u64; WORDS];
        selected.bits[entry.0 / 64] |= 1 << (entry.0 % 64);
        pending[entry.0 / 64] |= 1 << (entry.0 % 64);
        loop {
            let mut current = None;
            for (word, bits) in pending.iter_mut().enumerate() {
                selected.work = add(selected.work, 1)?;
                if *bits != 0 {
                    let bit = bits.trailing_zeros() as usize;
                    *bits &= *bits - 1;
                    current = Some(word * 64 + bit);
                    break;
                }
            }
            let Some(id) = current else { break };
            selected.work = add(selected.work, 1)?;
            for call in &functions[id].calls {
                selected.work = add(selected.work, 1)?;
                let target = call.target.0;
                if target >= functions.len() {
                    return Err(mismatch("native reachability target"));
                }
                let word = target / 64;
                let bit = 1u64 << (target % 64);
                if selected.bits[word] & bit == 0 {
                    selected.bits[word] |= bit;
                    pending[word] |= bit;
                }
            }
        }
        Ok(selected)
    }

    fn contains(self, function: hir::DefId) -> bool {
        self.bits
            .get(function.0 / 64)
            .is_some_and(|bits| bits & (1u64 << (function.0 % 64)) != 0)
    }
    fn work(self) -> usize {
        self.work
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(text: &str) -> source::resource_fixtures::CheckedSource {
        source::resource_fixtures::checked(text)
    }

    #[test]
    fn diamond_includes_both_branches_and_shared_callee_once() {
        let case = checked("struct T{value:i32} fn dead(x:T)->i32{return x.value;} fn leaf(x:T)->i32{return x.value;} fn left(x:T)->i32{return leaf(x);} fn right(x:T)->i32{return leaf(x);} fn main()->i32{let a=left(T{value:20});let b=right(T{value:22});return a+b;}");
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let selected = Reachable::build(&plan, case.entry).unwrap();
        assert!(!selected.contains(hir::DefId(0)));
        for id in 1..5 {
            assert!(selected.contains(hir::DefId(id)));
        }
        assert!(!selected.contains(hir::DefId(256)));
        assert!(selected.work() <= 5 * (WORDS + 2) + 4 + WORDS);
    }

    #[test]
    fn mutually_recursive_calls_terminate_and_include_the_entire_component() {
        let case = checked("struct T{value:i32} fn dead(x:T)->i32{return x.value;} fn even(x:&T,n:i32)->i32{if n==0{return x.value;}else{return odd(&*x,n-1);}} fn odd(x:&T,n:i32)->i32{if n==0{return x.value;}else{return even(&*x,n-1);}} fn main()->i32{let x=T{value:42};return even(&x,4);}");
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let selected = Reachable::build(&plan, case.entry).unwrap();
        assert!(!selected.contains(hir::DefId(0)));
        for id in 1..4 {
            assert!(selected.contains(hir::DefId(id)));
        }
        assert!(selected.work() <= 4 * (WORDS + 2) + 3 + WORDS);
    }

    #[test]
    fn word_boundaries_and_last_admitted_function_are_reachable() {
        let mut text = "struct T{value:i32}".to_owned();
        for id in 0..255 {
            let body = if matches!(id, 63 | 127) {
                format!("return f{}(x);", id + 1)
            } else {
                "return x.value;".to_owned()
            };
            text.push_str(&format!("fn f{id}(x:T)->i32{{{body}}}"));
        }
        text.push_str("fn main()->i32{let a=f63(T{value:10});let b=f127(T{value:20});let c=f254(T{value:12});return a+b+c;}");
        let case = checked(&text);
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let selected = Reachable::build(&plan, case.entry).unwrap();
        for id in 0..256 {
            assert_eq!(
                selected.contains(hir::DefId(id)),
                [63, 64, 127, 128, 254, 255].contains(&id),
                "function {id}"
            );
        }
    }

    #[test]
    fn hosted_function_limit_and_word_boundaries_are_not_narrowed_to_bounded_limits() {
        let mut text = "struct T{value:i32}".to_owned();
        for id in 0..1023 {
            let body = if matches!(id, 255 | 511) {
                format!("return f{}(x);", id + 1)
            } else {
                "return x.value;".to_owned()
            };
            text.push_str(&format!("fn f{id}(x:T)->i32{{{body}}}"));
        }
        text.push_str("fn main()->i32{let a=f255(T{value:10});let b=f511(T{value:20});let c=f1022(T{value:12});return a+b+c;}");
        let case = checked(&text);
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let selected = Reachable::build(&plan, case.entry).unwrap();
        for id in 0..1024 {
            assert_eq!(
                selected.contains(hir::DefId(id)),
                [255, 256, 511, 512, 1022, 1023].contains(&id),
                "function {id}"
            );
        }
        assert!(selected.work() <= 1024 * (WORDS + 2) + 5 + WORDS);
    }

    fn emit<R: EmittedFunctions>(
        case: &source::resource_fixtures::CheckedSource,
        limits: Limits,
    ) -> (Result<String, Box<Diagnostic>>, NativeMetrics) {
        let mut accounting = Accounting::default();
        let result = native_module_policy_accounted_with_selection::<R>(
            &case.witness,
            Some(case.entry),
            &case.sources,
            plan::MAX_FUEL,
            limits,
            NativeEntryPolicy::TypedResult,
            &mut accounting,
            None,
            false,
        );
        (result, accounting.metrics)
    }

    #[test]
    fn selected_bodies_have_exact_expansion_and_separately_charged_control_storage() {
        let case = checked("struct T{value:i32} fn dead(x:T)->i32{return x.value+1;} fn main()->i32{let x=T{value:42};return x.value;}");
        let limits = Limits::for_policy(NativeEntryPolicy::TypedResult);
        let (full, old) = emit::<Full>(&case, limits);
        let (selected, current) = emit::<Reachable>(&case, limits);
        let full = full.unwrap();
        let selected = selected.unwrap();
        assert_eq!(full.matches("define internal").count(), 2);
        assert_eq!(selected.matches("define internal").count(), 1);
        assert!(selected.len() < full.len());
        assert_eq!(old.message_bytes, current.message_bytes);
        assert_eq!(old.allocation_attempts, current.allocation_attempts);
        assert_eq!(
            current.emitter_transient_bound,
            old.emitter_transient_bound + CONTROL_BYTES
        );
        assert_eq!(
            current.count_expansions,
            current.transfer_cells + current.message_bytes
        );
        assert_eq!(current.count_expansions, current.render_expansions);
        assert_eq!(
            current.count_ordinary_visits,
            current.render_ordinary_visits
        );
        let mut denied = limits;
        denied.metadata_bytes = current.metadata_peak - 1;
        let (result, metrics) = emit::<Reachable>(&case, denied);
        assert!(result.unwrap_err().message.contains("emission metadata"));
        assert_eq!(metrics.count_bytes, 0);
    }

    #[test]
    fn resource_preflight_and_release_inventory_select_the_same_function_bodies() {
        use crate::frontend::build_config::ExecutionPolicy;
        use crate::frontend::declaration_index::SourceOwner;
        use crate::frontend::project::{ProjectLimits, ProjectSources};
        for (index, text) in [
            "use alloc::bytes::ByteBuf;use alloc::bytes::new;struct T{value:i32}fn unused()->(){let bytes=new();return;}fn main()->i32{let x=T{value:42};return x.value;}",
            "use alloc::bytes::ByteBuf;use alloc::bytes::new;struct T{value:i32}struct Holder{bytes:ByteBuf}fn unused()->(){let bytes=new();let mut first=Holder{bytes:bytes};let other=new();let next=Holder{bytes:other};first=next;return;}fn main()->i32{let x=T{value:42};return x.value;}",
        ].into_iter().enumerate() {
            let path = std::env::temp_dir().join(format!("oxid-native-reachability-resource-{}-{index}.ox", std::process::id()));
            std::fs::write(&path, text).unwrap();
            let loaded = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
            std::fs::remove_file(&path).unwrap();
            let project = loaded.unwrap();
            let (program, entry) = source::check_enum_source(SourceOwner::project(&project)).unwrap();
            let full = program.native_module_with_execution(entry, project.sources(), false, ExecutionPolicy::Bounded, None).unwrap();
            let selected = program.native_optimized_module_with_execution(entry, project.sources(), false, ExecutionPolicy::Bounded, None, false).unwrap();
            assert!(selected.len() < full.len());
            assert!(selected.matches("define internal").count() < full.matches("define internal").count());
        }
    }

    #[test]
    fn unreachable_over_budget_function_is_rejected_before_selection_or_output() {
        let text = format!("struct T{{value:i32}}fn unused()->(){{let x=T{{value:0}};{}return;}}fn main()->i32{{let x=T{{value:42}};return x.value;}}", "0;".repeat(253));
        let case = checked(&text);
        let limits = Limits::for_policy(NativeEntryPolicy::TypedResult);
        let (full, old) = emit::<Full>(&case, limits);
        let (selected, current) = emit::<Reachable>(&case, limits);
        let full = full.unwrap_err();
        let selected = selected.unwrap_err();
        assert_eq!(selected.code, full.code);
        assert_eq!(selected.message, full.message);
        assert_eq!(selected.primary, full.primary);
        assert_eq!(old.count_bytes, 0);
        assert_eq!(current.count_bytes, 0);
    }
}
