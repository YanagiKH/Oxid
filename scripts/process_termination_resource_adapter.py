"""Account for termination query storage in the retained scratch tests."""
import hashlib
from types import SimpleNamespace


PREDECESSOR = (30917, 'c6b1020b89464b85f61429b8da46dfe74d6b9321dd11706e8646595023b352be')
DERIVED = (31737, '7c8250cd359ced4dba72c9ff9295256763de94fddc057af75bd6458feeff927c')

MODEL = b"""#[allow(dead_code)]
#[repr(u8)]
enum ReviewProcessFunction { Enable, Requested }

#[allow(dead_code)]
struct ReviewProcessSelectors {
    builtin_set: [u8; 9],
    experimental_arguments: [[u8; 8]; 2],
    inventory_word: u64,
    inventory_bits: [u8; 2],
    inventory_construction_and_returns: [u8; 4],
    inventory_queries: [u8; 2],
    empty_result: bool,
    population_result: u32,
    count_result: usize,
    operation: u8,
    classifier_roles: [Option<ReviewProcessFunction>; 3],
    selector_roles: [bool; 2],
    surcharge: usize,
    checked_sum: Option<usize>,
    checked_result: [Result<usize, Box<crate::frontend::diagnostic::Diagnostic>>; 2],
}

"""
GROWTH = b'fn current_vector_review_fixed_growth() -> usize {'
SEAMS = (
    (GROWTH, MODEL + GROWTH),
    (b'    assert_eq!(catalog_controls, crate::frontend::builtin_catalog::builtin_set_control_bytes());',
     b'    let process_selectors = size_of::<ReviewProcessSelectors>();\n'
     b'    assert_eq!(catalog_controls + process_selectors, '
     b'crate::frontend::builtin_catalog::builtin_set_control_bytes());'),
    (b'let growth = header_growth + syntax_controls + catalog_controls;',
     b'let growth = header_growth + syntax_controls + catalog_controls + process_selectors;'),
    (b'assert_eq!(CURRENT_BASE_SCRATCH, 4096 + 16 + 93 + 104);',
     b'assert_eq!(process_selectors, 120);\n'
     b'        assert_eq!(CURRENT_BASE_SCRATCH, 4096 + 16 + 93 + 104 + 120);'),
)


def replace_exact(body, seams):
    for before, after in seams:
        if body.count(before) != 1:
            raise ValueError('termination index resource seam changed')
        body = body.replace(before, after, 1)
    return body


def bind(parent):
    def adapt(body):
        original = parent.adapt(body)
        if (len(original), hashlib.sha256(original).hexdigest()) != PREDECESSOR:
            raise ValueError('wrong retained index resource model')
        current = replace_exact(original, SEAMS)
        if (len(current), hashlib.sha256(current).hexdigest()) != DERIVED:
            raise ValueError('wrong termination index resource model')
        if replace_exact(current, ((after, before) for before, after in reversed(SEAMS))) != original:
            raise ValueError('termination index resource model failed exact inversion')
        return current

    def descriptor():
        return {**parent.descriptor(),
                'version': 'process-termination-index-resource-successor-1',
                'prior_model': {'bytes': PREDECESSOR[0], 'sha256': PREDECESSOR[1]},
                'derived': {'bytes': DERIVED[0], 'sha256': DERIVED[1]},
                'native_64_bit_components': {**parent.descriptor()['native_64_bit_components'],
                                             'termination_selectors': 120}}

    return SimpleNamespace(RESOURCE=parent.RESOURCE, adapt=adapt, descriptor=descriptor)
