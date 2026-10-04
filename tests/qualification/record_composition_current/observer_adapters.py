#!/usr/bin/env python3
"""Exact, reversible archived Unit3 adapters. No source or file writes.

This is not a replay driver. It accepts only the sealed historical inputs and
current scalar-store file listed below, produces a separate derived byte string,
and rejects either stale or already adapted input. Historical JSON is unchanged.
"""
import hashlib

VERSION = "oxid-record-composition-observer-adapters-v1"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


IDENTITIES = {'typing': {'original': {'path': 'tests/fixtures/fixed_array_source_unit3/typing-replay-v1/components/observer.rs', 'bytes': 36020, 'sha256': '48b406fc0250a55933fb5f8720a2f83d0f8518e9baa758faf8d13a982ba5dd29'}, 'derived': {'path': 'derived/typing-observer.rs', 'bytes': 36781, 'sha256': 'b2e2229a1d3a0beddf664f03a448e254e2a1db4696104ce06d237696d91bbbc2'}, 'substitutions': 8}, 'prepare': {'original': {'path': 'tests/fixtures/typed_project_unit3_independent/components/observer/prepare.py', 'bytes': 14411, 'sha256': 'b040216c2484e9d4fd28a725d3ea7af85f0c57bd889263996b26df87aff9f1d9'}, 'derived': {'path': 'derived/observer-prepare.py', 'bytes': 15484, 'sha256': 'ab6d92e3c46099270a788bed0204e53e10b75069871eaeaf9c53aea5408c7d36'}, 'substitutions': 1}, 'store': {'original': {'path': 'src/frontend/oir/owned/execute.rs', 'bytes': 80482, 'sha256': '6e5fbf2687161543f1b5d0b640dd182df30bd2b5c65e6dd7b12a54428ed976fe'}, 'derived': {'path': 'derived/owned-execute-store.rs', 'bytes': 81055, 'sha256': '90c9e3d55b7a410c9c8197878c70a44eda1e27b2af1ccb487414061312246e42'}, 'substitutions': 1}}

TYPING_SEAMS = (
    (b"struct V<'a, 's>(ValueTy, &'a di::DeclarationIndex<'s>, &'a source::SourceMap);",
     b'fn historical_field_scalar(ty: ValueTy) -> Result<&\'static str, String> {\n    match ty {\n        ValueTy::Scalar(ty) => Ok(scalar(ty)),\n        ValueTy::Owned(_) => Err("incomplete-observation: historical scalar record field required".into()),\n    }\n}\nfn historical_referent(referent: BorrowedTy) -> Result<AggregateTy, fmt::Error> {\n    match referent {\n        BorrowedTy::Exact(aggregate) => Ok(aggregate),\n        BorrowedTy::ScalarSlice(_) => Err(fmt::Error),\n    }\n}\nstruct V<\'a, \'s>(ValueTy, &\'a di::DeclarationIndex<\'s>, &\'a source::SourceMap);'),
    (b'ParameterTy::Reference { aggregate, kind } => write!(',
     b'ParameterTy::Reference { referent, kind } => write!('),
    (b'V(ValueTy::Owned(aggregate), self.1, self.2)',
     b'V(ValueTy::Owned(historical_referent(referent)?), self.1, self.2)'),
    (b'struct ProjectionRow(Option<Projection>);\nimpl fmt::Display for ProjectionRow {',
     b"struct ProjectionRow<'a>(Option<&'a Projection>);\nimpl fmt::Display for ProjectionRow<'_> {"),
    (b'        let (b, mode) = match p.base {',
     b'        // The frozen row has exactly one field slot; never flatten a path.\n        if p.path.as_slice() != [p.field] {\n            return Err(fmt::Error);\n        }\n        let (b, mode) = match p.base {'),
    (b'J(scalar(x.ty))',
     b'J(historical_field_scalar(x.ty)?)'),
    (b'display_alloc.observer_trace_bound(0)?;',
     b'display_alloc.observer_trace_bound(0).map_err(|_| "trace reserve failed")?;'),
    (b'a.observer_trace_bound(TRACE_ROWS)?;',
     b'a.observer_trace_bound(TRACE_ROWS).map_err(|_| "trace reserve failed")?;'),
)

PREPARE_SEAMS = ((b'    start=s.index(\'    fn store_field(\');end=s.index(\'    fn transfer_payload(\',start)\n    part=s[start:end]\n    part=insert_after(part,\'        let bytes = &mut self.frames[key.frame as usize].payload;\\n\',hook(\'let unit3_width=match value {Scalar::I32(_)=>4,Scalar::Bool(_)|Scalar::Unit=>1};\')+hook(\'let unit3_before=bytes.get(offset..offset+unit3_width).map(|v|v.to_vec());\'))\n    part=insert_before(part,\'        Ok(())\\n\',hook(f\'{J}::event("payload_write",&(key,field,value,span,unit3_before,bytes.get(offset..offset+unit3_width)));\'))\n    s=s[:start]+part+s[end:]\n',
                  b'    # Exact scalar store only. Preserve encode errors and emit after success.\n    s=replace(s,\'    fn store_field(\\n        &mut self,\\n        key: OwnerKey,\\n        field: FieldId,\\n        value: Scalar,\\n        span: Span,\\n    ) -> Result<()> {\\n        let (offset, ty) = self.payload_field(key, field, span)?;\\n        if ty != value.ty() {\\n            return Err(bad("field value type", span));\\n        }\\n        encode(\\n            &mut self.frames[key.frame as usize].payload,\\n            offset,\\n            value,\\n            span,\\n        )\\n    }\\n\',\'    fn store_field(\\n        &mut self,\\n        key: OwnerKey,\\n        field: FieldId,\\n        value: Scalar,\\n        span: Span,\\n    ) -> Result<()> {\\n        let (offset, ty) = self.payload_field(key, field, span)?;\\n        if ty != value.ty() {\\n            return Err(bad("field value type", span));\\n        }\\n        #[cfg(test)]\\n        let unit3_width = match value {\\n            Scalar::I32(_) => 4,\\n            Scalar::Bool(_) | Scalar::Unit => 1,\\n        };\\n        #[cfg(test)]\\n        let unit3_before = self.frames[key.frame as usize].payload\\n            .get(offset..offset + unit3_width).map(|bytes| bytes.to_vec());\\n        encode(\\n            &mut self.frames[key.frame as usize].payload,\\n            offset,\\n            value,\\n            span,\\n        )?;\\n        #[cfg(test)]\\n        crate::frontend::unit3_observer::event("payload_write", &(\\n            key, field, value, span, unit3_before,\\n            self.frames[key.frame as usize].payload.get(offset..offset + unit3_width),\\n        ));\\n        Ok(())\\n    }\\n\')\n'),)

STORE_SEAMS = ((b'    fn store_field(\n        &mut self,\n        key: OwnerKey,\n        field: FieldId,\n        value: Scalar,\n        span: Span,\n    ) -> Result<()> {\n        let (offset, ty) = self.payload_field(key, field, span)?;\n        if ty != value.ty() {\n            return Err(bad("field value type", span));\n        }\n        encode(\n            &mut self.frames[key.frame as usize].payload,\n            offset,\n            value,\n            span,\n        )\n    }\n',
                b'    fn store_field(\n        &mut self,\n        key: OwnerKey,\n        field: FieldId,\n        value: Scalar,\n        span: Span,\n    ) -> Result<()> {\n        let (offset, ty) = self.payload_field(key, field, span)?;\n        if ty != value.ty() {\n            return Err(bad("field value type", span));\n        }\n        #[cfg(test)]\n        let unit3_width = match value {\n            Scalar::I32(_) => 4,\n            Scalar::Bool(_) | Scalar::Unit => 1,\n        };\n        #[cfg(test)]\n        let unit3_before = self.frames[key.frame as usize].payload\n            .get(offset..offset + unit3_width).map(|bytes| bytes.to_vec());\n        encode(\n            &mut self.frames[key.frame as usize].payload,\n            offset,\n            value,\n            span,\n        )?;\n        #[cfg(test)]\n        crate::frontend::unit3_observer::event("payload_write", &(\n            key, field, value, span, unit3_before,\n            self.frames[key.frame as usize].payload.get(offset..offset + unit3_width),\n        ));\n        Ok(())\n    }\n'),)

def _identity(data, expected, label):
    require(isinstance(data, bytes), label + ": bytes required")
    require(len(data) == expected["bytes"] and digest(data) == expected["sha256"],
            label + ": exact identity differs")


def _derive(original, name, seams):
    identity = IDENTITIES[name]
    _identity(original, identity["original"], name + " original")
    result = original
    require(len(seams) == identity["substitutions"], name + ": substitution count")
    for old, new in seams:
        require(result.count(old) == 1, name + ": exact old seam cardinality")
        result = result.replace(old, new)
    _identity(result, identity["derived"], name + " derived")
    require(_reverse(result, name, seams) == original, name + ": reversal differs")
    return result


def _reverse(derived, name, seams):
    identity = IDENTITIES[name]
    _identity(derived, identity["derived"], name + " derived")
    result = derived
    for old, new in reversed(seams):
        require(result.count(new) == 1, name + ": exact new seam cardinality")
        result = result.replace(new, old)
    _identity(result, identity["original"], name + " original")
    return result


def derive_typing_observer(original):
    return _derive(original, "typing", TYPING_SEAMS)


def reverse_typing_observer(derived):
    return _reverse(derived, "typing", TYPING_SEAMS)


def derive_observer_prepare(original):
    return _derive(original, "prepare", PREPARE_SEAMS)


def reverse_observer_prepare(derived):
    return _reverse(derived, "prepare", PREPARE_SEAMS)


def derive_scalar_store_instrumentation(original):
    return _derive(original, "store", STORE_SEAMS)


def reverse_scalar_store_instrumentation(derived):
    return _reverse(derived, "store", STORE_SEAMS)

RAW_WALKER_SEAMS = (
    (b'pub(super) fn register(raw:&RawOwnedProgram) {\n',
     b'// This historical registry has no array, slice or composed-record schema.\n// Reject the complete raw input before capturing output or registering addresses.\nfn historical_instruction(value: &OwnedInstruction) -> bool {\n    match value {\n        OwnedInstruction::ConstructComposite { .. }\n        | OwnedInstruction::ReadProjection { .. }\n        | OwnedInstruction::WriteProjection { .. }\n        | OwnedInstruction::ProjectionLength { .. }\n        | OwnedInstruction::ConstructArray { .. }\n        | OwnedInstruction::ReadIndex { .. }\n        | OwnedInstruction::WriteIndex { .. }\n        | OwnedInstruction::ArrayLength { .. } => false,\n        OwnedInstruction::Scalar(_)\n        | OwnedInstruction::StorageLive(_)\n        | OwnedInstruction::StorageEnd(_)\n        | OwnedInstruction::Construct { .. }\n        | OwnedInstruction::MoveInitialize { .. }\n        | OwnedInstruction::Replace { .. }\n        | OwnedInstruction::Discard(_)\n        | OwnedInstruction::ReadField { .. }\n        | OwnedInstruction::WriteField { .. }\n        | OwnedInstruction::OpenCall(_)\n        | OwnedInstruction::PrepareScalar { .. }\n        | OwnedInstruction::PrepareOwned { .. }\n        | OwnedInstruction::PrepareBorrow { .. } => true,\n    }\n}\nfn historical_domain(raw: &RawOwnedProgram) {\n    const ERROR: &str = "OBSERVATION_UNSUPPORTED: historical scalar-record domain";\n    for record in &raw.records {\n        for field in &record.fields {\n            assert!(matches!(field.ty, ParameterTy::Value(ValueTy::Scalar(_))), "{ERROR}");\n        }\n    }\n    for function in &raw.functions {\n        assert!(matches!(function.result, ValueTy::Scalar(_) | ValueTy::Owned(AggregateTy::Record(_))), "{ERROR}");\n        for owner in &function.owners {\n            assert!(matches!(owner.aggregate(), AggregateTy::Record(_)), "{ERROR}");\n        }\n        for reference in &function.references {\n            assert!(matches!(reference.referent(), BorrowedTy::Exact(AggregateTy::Record(_))), "{ERROR}");\n        }\n        for loan in &function.loans {\n            assert!(matches!(loan.referent(), BorrowedTy::Exact(AggregateTy::Record(_))), "{ERROR}");\n        }\n        for block in &function.blocks {\n            for statement in &block.statements {\n                assert!(historical_instruction(&statement.kind), "{ERROR}");\n            }\n        }\n    }\n}\npub(super) fn register(raw:&RawOwnedProgram) {\n    if !journal::enabled() { return; }\n    historical_domain(raw);\n'),
    (b'                    OwnedInstruction::Scalar(v)=>statement(v,&format!("{p}.kind.scalar")),\n',
     b'                    OwnedInstruction::ConstructComposite { .. }\n                    | OwnedInstruction::ReadProjection { .. }\n                    | OwnedInstruction::WriteProjection { .. }\n                    | OwnedInstruction::ProjectionLength { .. }\n                    | OwnedInstruction::ConstructArray { .. }\n                    | OwnedInstruction::ReadIndex { .. }\n                    | OwnedInstruction::WriteIndex { .. }\n                    | OwnedInstruction::ArrayLength { .. } => panic!("OBSERVATION_UNSUPPORTED: historical scalar-record domain"),\n                    OwnedInstruction::Scalar(v)=>statement(v,&format!("{p}.kind.scalar")),\n'),
)
IDENTITIES['raw_walker'] = {'original': {'path': 'src/frontend/oir/owned/unit3_raw_owned.rs', 'bytes': 3309, 'sha256': '07852e6e613a292f55b34a53bd0f1c82b343cb2323a5f23be0affb184df7742d'}, 'derived': {'path': 'derived/unit3_raw_owned.rs', 'bytes': 6279, 'sha256': '5d8b42dab3a707af67e0e3a2972bdfcca901a85792821a800e466f313ac4e870'}, 'substitutions': 2, 'archived_container': {'path': 'tests/fixtures/typed_project_unit3_independent/components/observer/overlay-v5.patch', 'bytes': 45496, 'sha256': '62a2b6382f7684c5aaf03c355580f2e8d582c6c2688d7bab970f0b50f9c0ed54'}}

def derive_raw_walker(original):
    return _derive(original, 'raw_walker', RAW_WALKER_SEAMS)

def reverse_raw_walker(derived):
    return _reverse(derived, 'raw_walker', RAW_WALKER_SEAMS)

JOURNAL_SEAMS = (
    (b'pub(super) fn registry_begin() {',
     b'pub(super) fn enabled() -> bool {\n    JOURNAL.with_borrow(|journal| journal.enabled)\n}\npub(super) fn registry_begin() {'),
)
IDENTITIES['journal'] = {'original': {'path': 'src/frontend/unit3_observer.rs', 'bytes': 9372, 'sha256': '3654621dd6f173c18f4416d6cdd999273431f774808bdec51785f911e517a543'}, 'derived': {'path': 'derived/unit3_observer.rs', 'bytes': 9459, 'sha256': '68b72b333273db47158b01cb55ec81f1ccd309f6c5519c3a07d06f10caf22ef9'}, 'substitutions': 1, 'archived_container': {'path': 'tests/fixtures/typed_project_unit3_independent/components/observer/overlay-v5.patch', 'bytes': 45496, 'sha256': '62a2b6382f7684c5aaf03c355580f2e8d582c6c2688d7bab970f0b50f9c0ed54'}}

def derive_journal(original):
    return _derive(original, 'journal', JOURNAL_SEAMS)

def reverse_journal(derived):
    return _reverse(derived, 'journal', JOURNAL_SEAMS)


def archived_observer_additions(original_patch):
    """Extract the three exact archived added modules, without applying patches."""
    _identity(original_patch, IDENTITIES["raw_walker"]["archived_container"], "archived overlay")
    paths = ("src/frontend/unit3_observer.rs", "src/frontend/oir/unit3_raw_scalar.rs",
             "src/frontend/oir/owned/unit3_raw_owned.rs")
    lines = original_patch.decode("utf-8").splitlines(keepends=True)
    result = {}
    for path in paths:
        marker = "+++ b/" + path + "\n"
        require(lines.count(marker) == 1, "archived addition cardinality")
        start = lines.index(marker)
        require(lines[start - 1] == "--- /dev/null\n", "not an archived addition")
        start += 2
        content = []
        while start < len(lines) and not lines[start].startswith("--- "):
            require(lines[start].startswith("+"), "unexpected archived addition record")
            content.append(lines[start][1:])
            start += 1
        result[path] = "".join(content).encode("utf-8")
    _identity(result[paths[0]], IDENTITIES["journal"]["original"], "archived journal")
    _identity(result[paths[2]], IDENTITIES["raw_walker"]["original"], "archived raw walker")
    return result
