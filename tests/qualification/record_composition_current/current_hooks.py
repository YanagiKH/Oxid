#!/usr/bin/env python3
"""Sealed current-only hook rebinding; no changes to frozen artifacts."""
import observer_adapters as adapter

IDENTITIES = {'src/frontend/oir/owned/array_observe.rs': {'original': {'path': 'src/frontend/oir/owned/array_observe.rs', 'bytes': 16256, 'sha256': '59289c38e1e89d28fc5563e643fcaab12586c39ce4c48f46d3d7dcaa0a1137af'}, 'derived': {'path': 'src/frontend/oir/owned/array_observe.rs', 'bytes': 16504, 'sha256': '2a42af5b87d738630de34cf392ede82d4a8c0ad0ae0ce51deb658dc5fa9a8d85'}, 'substitutions': 1}, 'src/frontend/declaration_index/resource.rs': {'original': {'path': 'src/frontend/declaration_index/resource.rs', 'bytes': 13343, 'sha256': '5f9201347edfba22a57f0020c61ebcf8080fda0249fab0322b0dfb35c9139e9f'}, 'derived': {'path': 'src/frontend/declaration_index/resource.rs', 'bytes': 13430, 'sha256': 'b8cc717f2863924e796316c96084c5133b93e22883ea9fc0e304f27feb86a59e'}, 'substitutions': 1}, 'src/frontend/project/budget.rs': {'original': {'path': 'src/frontend/project/budget.rs', 'bytes': 7740, 'sha256': 'b4bfc05194f785cb9af42d6d1dcc1b4d847259befd54abd585e41eccece5524b'}, 'derived': {'path': 'src/frontend/project/budget.rs', 'bytes': 8151, 'sha256': '0040fa45e97a4d45c6e5d8cafcb6a39e23e2c704ddd915fbd618e098c727503f'}, 'substitutions': 2}, 'src/frontend/project.rs': {'original': {'path': 'src/frontend/project.rs', 'bytes': 35585, 'sha256': '7931b70180c72e99488709bd8d05cf673dd32d29c4091883e608a03a91d18f85'}, 'derived': {'path': 'src/frontend/project.rs', 'bytes': 35861, 'sha256': '3dd4af8d361083599bd80599225017f2b2748687a8d662b6504bedbe9eef6166'}, 'substitutions': 3}, 'src/frontend/oir/owned/source/mod.rs': {'original': {'path': 'src/frontend/oir/owned/source/mod.rs', 'bytes': 818, 'sha256': 'aa83e332b8bf646cd83c4749d6d56e1d8fa9f936b36e0bf211c8ab94e0855ae4'}, 'derived': {'path': 'src/frontend/oir/owned/source/mod.rs', 'bytes': 859, 'sha256': '0790e70154c45e25f0b0ea172163b373b932041794207c4a37c66279cc30cd82'}, 'substitutions': 1}}

SEAMS = {'src/frontend/oir/owned/array_observe.rs': [(b'    pub(super) fn record_event(&mut self, event: Event) {\n        if !self.observer.enabled {\n            self.events.push(event);\n            return;\n        }\n        if !self.observer.collecting() {\n            return;\n        }\n        if self\n            .events\n            .len()\n            .checked_add(1)\n            .and_then(|n| n.checked_mul(size_of::<Event>()))\n            .is_none_or(|n| n > MAX_EVENTS * 80)\n            || self.events.len() >= MAX_EVENTS\n            || self\n                .observer\n                .reject_allocation(ObservationAllocationSite::Event)\n            || self.events.try_reserve_exact(1).is_err()\n        {\n            self.observer.truncated = true;\n            return;\n        }\n        self.events.push(event);\n    }\n', b'    pub(super) fn record_event(&mut self, event: Event) {\n        if !self.observer.enabled {\n            self.events.push(event);\n            crate::frontend::unit3_observer::event("existing_owned_event", self.events.last().expect("just appended event"));\n            return;\n        }\n        if !self.observer.collecting() {\n            return;\n        }\n        if self\n            .events\n            .len()\n            .checked_add(1)\n            .and_then(|n| n.checked_mul(size_of::<Event>()))\n            .is_none_or(|n| n > MAX_EVENTS * 80)\n            || self.events.len() >= MAX_EVENTS\n            || self\n                .observer\n                .reject_allocation(ObservationAllocationSite::Event)\n            || self.events.try_reserve_exact(1).is_err()\n        {\n            self.observer.truncated = true;\n            return;\n        }\n        self.events.push(event);\n        crate::frontend::unit3_observer::event("existing_owned_event", self.events.last().expect("just appended event"));\n    }\n')], 'src/frontend/declaration_index/resource.rs': [(b'    pub fn used(&self) -> u64 {', b"    #[cfg(test)]\n    pub fn observer_phase(&self) -> &'static str { self.phase.get() }\n    pub fn used(&self) -> u64 {")], 'src/frontend/project/budget.rs': [(b'    pub observer_trace_overflow: bool,', b'    pub observer_trace_overflow: bool,\n    #[cfg(test)]\n    pub observer_loader_phases: [usize; 4],'), (b'impl Allocator {\n', b'impl Allocator {\n    #[cfg(test)]\n    pub fn observer_loader_mark(&mut self, slot: usize) {\n        if self.observer_trace_limit.is_some() {\n            match self.observer_loader_phases[slot].checked_add(1) {\n                Some(n) => self.observer_loader_phases[slot] = n,\n                None => self.observer_trace_overflow = true,\n            }\n        }\n    }\n')], 'src/frontend/project.rs': [(b'        let tokens = lexer::lex_with_limit(source, remaining_tokens).map_err(one)?;', b'        #[cfg(test)]\n        self.allocator.observer_loader_mark(0);\n        let tokens = lexer::lex_with_limit(source, remaining_tokens).map_err(one)?;\n        #[cfg(test)]\n        self.allocator.observer_loader_mark(1);'), (b'        let (program, nodes) = if self.arrays == parser::ArraySyntaxPolicy::Closed {', b'        #[cfg(test)]\n        self.allocator.observer_loader_mark(2);\n        let (program, nodes) = if self.arrays == parser::ArraySyntaxPolicy::Closed {'), (b'        self.project.usage.syntax_nodes =\n', b'        #[cfg(test)]\n        self.allocator.observer_loader_mark(3);\n        self.project.usage.syntax_nodes =\n')], 'src/frontend/oir/owned/source/mod.rs': [(b'_controls;\n\n#[cfg(test)]\nmod array_pipeline;\n\n#[cfg(test)]\nmod array_consumer_tests;\n\n#[cfg(test)]\nmod slice_raw_tests;\n#[cfg(test)]\nmod slice_tests;\n', b'_controls;\n\n#[cfg(test)]\nmod array_pipeline;\n\n#[cfg(test)]\nmod array_consumer_tests;\n\n#[cfg(test)]\nmod slice_raw_tests;\n#[cfg(test)]\nmod slice_tests;\n\n#[cfg(test)]\nmod array_typing_observer;\n')]}

def derive(path, original):
    adapter.require(path in IDENTITIES, "unknown current test-hook path")
    adapter._identity(original, IDENTITIES[path]["original"], path)
    result = original
    for old, new in SEAMS[path]:
        adapter.require(result.count(old) == 1, "test-hook old seam cardinality")
        result = result.replace(old, new)
    adapter._identity(result, IDENTITIES[path]["derived"], path)
    adapter.require(reverse(path, result) == original, "test-hook reversal differs")
    return result


def reverse(path, derived):
    adapter.require(path in IDENTITIES, "unknown current test-hook path")
    adapter._identity(derived, IDENTITIES[path]["derived"], path)
    result = derived
    for old, new in reversed(SEAMS[path]):
        adapter.require(result.count(new) == 1, "test-hook new seam cardinality")
        result = result.replace(new, old)
    adapter._identity(result, IDENTITIES[path]["original"], path)
    return result


TYPING_PATHS = ("src/frontend/declaration_index/resource.rs", "src/frontend/project/budget.rs",
                "src/frontend/project.rs", "src/frontend/oir/owned/source/mod.rs")
EVENT_PATH = "src/frontend/oir/owned/array_observe.rs"


def derive_typing_overlay(originals, original_observer):
    adapter.require(set(originals) == set(TYPING_PATHS), "typing source membership differs")
    result = {path: derive(path, data) for path, data in originals.items()}
    result["src/frontend/oir/owned/source/array_typing_observer.rs"] = adapter.derive_typing_observer(original_observer)
    return result
