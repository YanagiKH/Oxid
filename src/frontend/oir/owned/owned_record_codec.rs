//! Canonical active cells for finite, non-Copy record vector elements.
//!
//! This codec accepts only an independently checked declaration facade. It
//! never copies record padding or inactive enum payloads, and it never treats
//! a scalar integer as resource authority. Child resources retain their own
//! entries in the execution heap, including while a vector growth is staged.
use super::{
    byte_buffer::{CheckedVectorElement, Failure, Heap, RecordVectorElement},
    vector_type, AggregateTy, Declarations, RecordId, ValueTy, MAX_CONTAINMENT_DEPTH,
};
use crate::frontend::{
    hir,
    oir::owned_types::{OwnedVectorRoot, OwnedVectorRootSlot},
};

#[path = "owned_record_bounds.rs"]
pub(super) mod bounds;

type Result<T> = std::result::Result<T, Failure>;
type Stack = [Option<(ValueTy, usize, usize)>; MAX_CONTAINMENT_DEPTH + 1];

/// A record identity and its ordinary checked layout. This is a separate
/// domain from the dense Copy-record vector codec.
#[derive(Clone, Copy)]
pub(super) struct Element<'a> {
    declarations: &'a Declarations,
    root: OwnedVectorRootSlot,
    bytes: usize,
    visits: usize,
    resources: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Scalar(hir::Ty),
    Resource(AggregateTy),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    offset: usize,
    bytes: usize,
    kind: Kind,
}

/// Fixed-depth traversal of the active value. Enum payload authority comes
/// from the checked tag and declaration, rather than a stored pointer/plan.
struct Cells<'declarations, 'bytes> {
    element: Element<'declarations>,
    bytes: &'bytes [u8],
    stack: Stack,
    length: usize,
}

impl<'a> Element<'a> {
    /// Component-test convenience. Production consumers obtain elements from
    /// one preadmitted checked table; this allocating factory is never used by
    /// a heap primitive or an allocation-free push/pop operation.
    #[cfg(test)]
    pub(super) fn check(declarations: &'a Declarations, record: RecordId) -> Result<Self> {
        bounds::CheckedElements::check(declarations, super::plan::MAX_PLAN_BYTES)
            .map_err(|_| Failure::Descriptor)?
            .element(record)
            .map_err(|_| Failure::Descriptor)
    }

    pub(super) const fn record(self) -> RecordId {
        match self.root.root() {
            OwnedVectorRoot::Record(id) => id,
            OwnedVectorRoot::Enum(_) => panic!("enum elements have no record ID"),
        }
    }

    pub(super) const fn root(self) -> OwnedVectorRoot {
        self.root.root()
    }

    pub(super) const fn storage_key(self) -> u32 {
        self.root.raw()
    }

    pub(super) const fn declarations(self) -> &'a Declarations {
        self.declarations
    }

    pub(super) const fn size(self) -> usize {
        self.bytes
    }

    /// Checked maximum over active enum alternatives, summing record fields.
    pub(super) const fn identity_capacity(self) -> usize {
        self.resources
    }

    /// Exact maximum iterator transitions for the finite checked type. This
    /// uses the active alternative maximum, never stride times a depth cap.
    pub(super) fn traversal_work(self, records: usize) -> Option<usize> {
        self.visits.checked_mul(records)
    }

    fn cells(self, bytes: &[u8]) -> Result<Cells<'a, '_>> {
        if bytes.len() != self.bytes {
            return Err(Failure::Descriptor);
        }
        let mut stack = [None; MAX_CONTAINMENT_DEPTH + 1];
        stack[0] = Some((ValueTy::Owned(self.root().aggregate()), 0, 0));
        Ok(Cells {
            element: self,
            bytes,
            stack,
            length: 1,
        })
    }

    /// Validate shape, scalar representations and registered resource kinds.
    /// The caller supplies its independently admitted identity scratch; no
    /// allocation or implicit retry occurs here. The returned count can be
    /// combined with other records before checking ownership uniqueness.
    pub(super) fn preflight(
        self,
        bytes: &[u8],
        heap: &Heap,
        identities: &mut [u64],
    ) -> Result<usize> {
        let mut count = 0usize;
        for cell in self.cells(bytes)? {
            let cell = cell?;
            let source = &bytes[cell.offset..cell.offset + cell.bytes];
            match cell.kind {
                Kind::Scalar(ty) => {
                    let canonical = match ty {
                        hir::Ty::Bool => source[0] <= 1,
                        hir::Ty::Unit => source[0] == 0,
                        hir::Ty::Char => char::from_u32(u32::from_le_bytes(
                            source.try_into().map_err(|_| Failure::Descriptor)?,
                        ))
                        .is_some(),
                        _ => true,
                    };
                    if !canonical {
                        return Err(Failure::Descriptor);
                    }
                }
                Kind::Resource(aggregate) => {
                    match aggregate {
                        AggregateTy::ByteBuf => {
                            heap.descriptor(source)?;
                        }
                        AggregateTy::File => {
                            heap.file_descriptor(source)?;
                        }
                        AggregateTy::Vector(ty) => {
                            heap.vector_descriptor(
                                source,
                                CheckedVectorElement::Scalar(vector_type::element(ty)),
                            )?;
                        }
                        AggregateTy::RecordVector(record) => {
                            heap.vector_descriptor(
                                source,
                                CheckedVectorElement::Record(RecordVectorElement::check(
                                    self.declarations,
                                    record,
                                )?),
                            )?;
                        }
                        _ => return Err(Failure::Descriptor),
                    }
                    let identity = u64::from_le_bytes(
                        source[..8].try_into().map_err(|_| Failure::Descriptor)?,
                    );
                    if identity != 0 {
                        *identities.get_mut(count).ok_or(Failure::Descriptor)? = identity;
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// Prepare canonical bytes before a commit. Source and destination are
    /// distinct admitted storage; a failure leaves only scratch modified.
    /// Logical ownership is transferred separately by the verified consumer.
    pub(super) fn canonical_copy(self, source: &[u8], destination: &mut [u8]) -> Result<()> {
        if destination.len() != self.bytes {
            return Err(Failure::Descriptor);
        }
        // Establish the entire active shape before touching destination.
        for cell in self.cells(source)? {
            cell?;
        }
        destination.fill(0);
        for cell in self.cells(source)? {
            let cell = cell?;
            let extent = cell.offset..cell.offset + cell.bytes;
            destination[extent.clone()].copy_from_slice(&source[extent]);
        }
        Ok(())
    }

    /// Every child was validated, including uniqueness, before entering the
    /// release phase. This phase performs no allocation or fuel-dependent work.
    pub(super) fn release_children(self, bytes: &[u8], heap: &mut Heap) {
        for cell in self.cells(bytes).expect("preflighted owned record shape") {
            let cell = cell.expect("preflighted owned record cell");
            let Kind::Resource(aggregate) = cell.kind else {
                continue;
            };
            let source = &bytes[cell.offset..cell.offset + cell.bytes];
            match aggregate {
                AggregateTy::ByteBuf => {
                    let mut value = heap.descriptor(source).expect("preflighted byte owner");
                    heap.release(&mut value).expect("unique byte owner");
                }
                AggregateTy::File => {
                    let mut value = heap
                        .file_descriptor(source)
                        .expect("preflighted file owner");
                    // Closing a valid file can report an OS error, but it still
                    // consumes its registry entry. Drop ignores that OS result.
                    let _ = heap.file_close(&mut value).expect("unique file owner");
                }
                AggregateTy::Vector(_) | AggregateTy::RecordVector(_) => {
                    let ty = vector_type::VectorTy::from_aggregate(aggregate)
                        .expect("preflighted Copy vector kind");
                    let element = vector_type::checked(ty, self.declarations)
                        .expect("preflighted Copy vector type");
                    let mut value = heap
                        .vector_descriptor(source, element)
                        .expect("preflighted Copy vector owner");
                    heap.vector_release(&mut value, element)
                        .expect("unique Copy vector owner");
                }
                _ => unreachable!("preflighted resource kind"),
            }
        }
    }
}

/// The unstable slice sort performs no allocation and takes O(n log n) work.
/// A zero identity is never stored: empty descriptors do not own a resource.
pub(super) fn unique_identities(identities: &mut [u64]) -> Result<()> {
    identities.sort_unstable();
    if identities.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Failure::Descriptor);
    }
    Ok(())
}

impl Cells<'_, '_> {
    fn push(&mut self, value: ValueTy, offset: usize) -> Result<()> {
        if self.length == self.stack.len() {
            return Err(Failure::Descriptor);
        }
        self.stack[self.length] = Some((value, offset, 0));
        self.length += 1;
        Ok(())
    }

    fn cell(&self, offset: usize, bytes: usize, kind: Kind) -> Result<Cell> {
        if offset
            .checked_add(bytes)
            .is_none_or(|end| end > self.bytes.len())
        {
            return Err(Failure::Descriptor);
        }
        Ok(Cell {
            offset,
            bytes,
            kind,
        })
    }

    fn scalar(&self, offset: usize, ty: hir::Ty) -> Result<Cell> {
        self.cell(offset, vector_type::element(ty).size(), Kind::Scalar(ty))
    }

    fn advance(&mut self) -> Result<Option<Cell>> {
        while self.length != 0 {
            let position = self.length - 1;
            let (value, relative, next) = self.stack[position].ok_or(Failure::Descriptor)?;
            match value {
                ValueTy::Owned(
                    AggregateTy::OwnedRecordVector(_) | AggregateTy::OwnedEnumVector(_),
                ) => return Err(Failure::Descriptor),
                ValueTy::Scalar(ty) => {
                    self.length -= 1;
                    return self.scalar(relative, ty).map(Some);
                }
                ValueTy::Owned(
                    aggregate @ (AggregateTy::ByteBuf
                    | AggregateTy::File
                    | AggregateTy::Vector(_)
                    | AggregateTy::RecordVector(_)),
                ) => {
                    self.length -= 1;
                    return self.cell(relative, 16, Kind::Resource(aggregate)).map(Some);
                }
                ValueTy::Owned(AggregateTy::FixedArray(array)) => {
                    if next == array.length().max(1) {
                        self.length -= 1;
                    } else {
                        self.stack[position] = Some((value, relative, next + 1));
                        let offset = next
                            .checked_mul(array.stride())
                            .and_then(|offset| relative.checked_add(offset))
                            .ok_or(Failure::Descriptor)?;
                        return self.scalar(offset, array.element()).map(Some);
                    }
                }
                ValueTy::Owned(AggregateTy::Record(record)) => {
                    let fields = self
                        .element
                        .declarations
                        .fields(record)
                        .map_err(|_| Failure::Descriptor)?;
                    if fields.is_empty() && next == 0 {
                        self.length -= 1;
                        return self.scalar(relative, hir::Ty::Unit).map(Some);
                    }
                    if let Some(field) = fields.get(next) {
                        self.stack[position] = Some((value, relative, next + 1));
                        self.push(
                            field.value_ty(),
                            relative
                                .checked_add(field.offset())
                                .ok_or(Failure::Descriptor)?,
                        )?;
                    } else {
                        self.length -= 1;
                    }
                }
                ValueTy::Owned(AggregateTy::Enum(enumeration)) => {
                    if next != 0 {
                        self.length -= 1;
                        continue;
                    }
                    let tag = self.scalar(relative, hir::Ty::I32)?;
                    let word = self
                        .bytes
                        .get(relative..relative + 4)
                        .ok_or(Failure::Descriptor)?;
                    let variant = self
                        .element
                        .declarations
                        .enums()
                        .variant_for_tag(
                            enumeration,
                            u32::from_le_bytes(word.try_into().unwrap()) as usize,
                        )
                        .map_err(|_| Failure::Descriptor)?;
                    self.stack[position] = Some((value, relative, 1));
                    if let Some(payload) = variant.value_payload() {
                        let offset = variant
                            .payload_offset()
                            .and_then(|offset| relative.checked_add(offset))
                            .ok_or(Failure::Descriptor)?;
                        self.push(payload, offset)?;
                    }
                    return Ok(Some(tag));
                }
            }
        }
        Ok(None)
    }
}
impl Iterator for Cells<'_, '_> {
    type Item = Result<Cell>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.advance() {
            Ok(Some(cell)) => Some(Ok(cell)),
            Ok(None) => None,
            Err(error) => {
                self.length = 0;
                Some(Err(error))
            }
        }
    }
}

/// Separate accounting bank for the new codec. The old Copy-record codec and
/// its request formulas retain their existing carriers and byte totals.
pub(super) const fn control_bytes() -> usize {
    5 * std::mem::size_of::<Cells<'static, 'static>>()
        + 2 * std::mem::size_of::<Stack>()
        + 8 * std::mem::size_of::<Result<Cell>>()
        + 4 * std::mem::size_of::<Element<'static>>()
        + 16 * std::mem::size_of::<usize>()
        + 2 * std::mem::size_of::<Result<usize>>()
        + std::mem::size_of::<&mut [u64]>()
        + 2 * std::mem::size_of::<OwnedVectorRoot>()
}
