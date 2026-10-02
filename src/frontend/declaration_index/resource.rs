//! Requested row storage and explicit work, independent of allocator capacity.
use super::*;
use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct IndexLimits {
    pub retained: u64,
    pub scratch: u64,
    pub work: u64,
}
impl Default for IndexLimits {
    fn default() -> Self {
        Self {
            retained: 32 * 1024 * 1024,
            scratch: 16 * 1024 * 1024,
            work: 256_000_000,
        }
    }
}
impl IndexLimits {
    fn lowered(self) -> Self {
        let d = Self::default();
        Self {
            retained: self.retained.min(d.retained),
            scratch: self.scratch.min(d.scratch),
            work: self.work.min(d.work),
        }
    }
}

#[derive(Debug)]
pub(in crate::frontend) struct WorkMeter {
    used: Cell<u64>,
    limit: Cell<u64>,
    #[cfg(test)]
    pub events: RefCell<Vec<WorkEvent>>,
    #[cfg(test)]
    pub observations: RefCell<Vec<Observation>>,
    #[cfg(test)]
    observing: Cell<bool>,
    #[cfg(test)]
    phase: Cell<&'static str>,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct WorkEvent {
    pub operation: &'static str,
    pub origin: Span,
    pub units: u64,
}
impl WorkMeter {
    pub fn new(limit: u64) -> Self {
        Self {
            used: Cell::new(0),
            limit: Cell::new(limit.min(IndexLimits::default().work)),
            #[cfg(test)]
            events: RefCell::new(Vec::new()),
            #[cfg(test)]
            observations: RefCell::new(Vec::new()),
            #[cfg(test)]
            observing: Cell::new(false),
            #[cfg(test)]
            phase: Cell::new("collection"),
        }
    }
    pub fn used(&self) -> u64 {
        self.used.get()
    }
    pub fn limit(&self) -> u64 {
        self.limit.get()
    }
    pub(super) fn restrict(&self, limit: u64) {
        self.limit.set(self.limit.get().min(limit));
    }
    /// Count-only preflight cannot choose W before the required I/J gates.
    /// Arithmetic is still checked and charged before each inspection; only
    /// the comparison with W is deferred to the ordered admission below.
    pub(super) fn preflight(&self, origin: Span) -> Result<(), Box<Diagnostic>> {
        let next = self
            .used
            .get()
            .checked_add(1)
            .ok_or_else(|| overflow(origin))?;
        self.used.set(next);
        #[cfg(test)]
        if self.observing.get() {
            self.events.borrow_mut().push(WorkEvent {
                operation: "preflight visit",
                origin,
                units: 1,
            });
        }
        Ok(())
    }
    pub fn debit(
        &self,
        units: u64,
        origin: Span,
        operation: &'static str,
    ) -> Result<(), Box<Diagnostic>> {
        let next = self
            .used
            .get()
            .checked_add(units)
            .ok_or_else(|| overflow(origin))?;
        if next > self.limit.get() {
            return Err(resource("declaration index work limit exceeded", origin));
        }
        self.used.set(next);
        #[cfg(test)]
        if self.observing.get() {
            self.events.borrow_mut().push(WorkEvent {
                operation,
                origin,
                units,
            });
        }
        #[cfg(not(test))]
        let _ = operation;
        Ok(())
    }
    pub fn phase(&self, phase: &'static str) {
        #[cfg(test)]
        self.phase.set(phase);
        #[cfg(test)]
        self.observe(Observation::Phase(phase));
        #[cfg(not(test))]
        let _ = phase;
    }
    #[cfg(test)]
    pub fn enable_observation(&self) {
        self.observing.set(true);
    }
    #[cfg(test)]
    pub fn observing(&self) -> bool {
        self.observing.get()
    }
    #[cfg(test)]
    pub fn observe(&self, event: Observation) {
        if self.observing.get() {
            self.observations.borrow_mut().push(event);
        }
    }
    pub fn signature_start(&self, function: DefId, origin: Span) {
        #[cfg(test)]
        self.observe(Observation::SignatureStart { function, origin });
        #[cfg(not(test))]
        let _ = (function, origin);
    }
    pub fn record_start(&self, record: RecordId, origin: Span) {
        #[cfg(test)]
        self.observe(Observation::RecordStart { record, origin });
        #[cfg(not(test))]
        let _ = (record, origin);
    }
    pub fn record_error(&self, error: &Diagnostic) {
        #[cfg(test)]
        if self.observing.get() {
            self.observe(Observation::Diagnostic {
                phase: self.phase.get(),
                diagnostic: error.clone(),
            });
        }
        #[cfg(not(test))]
        let _ = error;
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(in crate::frontend) enum Observation {
    Route {
        owned: bool,
    },
    Phase(&'static str),
    SignatureStart {
        function: DefId,
        origin: Span,
    },
    RecordStart {
        record: RecordId,
        origin: Span,
    },
    Plan(IndexPlan),
    Original {
        kind: &'static str,
        id: usize,
        module: ModuleId,
        name: Span,
        local: usize,
    },
    Import {
        id: usize,
        module: ModuleId,
        alias: Span,
        committed: bool,
        ty: Option<RecordId>,
        value: Option<DefId>,
        aliases: Vec<AliasObservation>,
        seen: Vec<SeenObservation>,
    },
    Frozen {
        root_main: Option<DefId>,
    },
    Target {
        operation: &'static str,
        origin: Span,
        kind: &'static str,
        id: usize,
    },
    Expression {
        function: DefId,
        origin: Span,
        ty: ValueTy,
        field: Option<FieldId>,
    },
    Binding {
        function: DefId,
        origin: Span,
        ty: super::super::oir::owned_types::ParameterTy,
    },
    Diagnostic {
        phase: &'static str,
        diagnostic: Diagnostic,
    },
    Projection {
        operation: &'static str,
        function: DefId,
        origin: Span,
        field: FieldId,
        ty: ValueTy,
    },
    BorrowArgument {
        function: DefId,
        origin: Span,
        binding: usize,
        ty: super::super::oir::owned_types::ParameterTy,
    },
    SortEmit {
        width: usize,
        position: usize,
        original: u32,
    },
}
#[cfg(test)]
#[derive(Clone, Debug)]
pub(in crate::frontend) struct AliasObservation {
    pub module: ModuleId,
    pub alias: Span,
    pub ty: Option<RecordId>,
    pub value: Option<DefId>,
    pub type_first: Option<usize>,
    pub value_first: Option<usize>,
}
#[cfg(test)]
#[derive(Clone, Debug)]
pub(in crate::frontend) struct SeenObservation {
    pub module: ModuleId,
    pub group: usize,
    pub type_first: Option<usize>,
    pub value_first: Option<usize>,
}
impl Default for WorkMeter {
    fn default() -> Self {
        Self::new(IndexLimits::default().work)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend) struct Counts {
    pub originals: u64,
    pub functions: u64,
    pub records: u64,
    pub fields: u64,
    pub modules: u64,
    pub imports: u64,
    pub original_bytes: u64,
    pub alias_bytes: u64,
    pub path_weight: u64,
}
#[derive(Clone, Copy, Debug)]
pub(in crate::frontend) struct IndexPlan {
    pub counts: Counts,
    pub retained: u64,
    pub scratch: u64,
    pub build_work: u64,
}

pub(super) fn add(a: u64, b: u64, at: Span) -> Result<u64, Box<Diagnostic>> {
    a.checked_add(b).ok_or_else(|| overflow(at))
}
fn mul(a: u64, b: u64, at: Span) -> Result<u64, Box<Diagnostic>> {
    a.checked_mul(b).ok_or_else(|| overflow(at))
}
pub(super) fn compact(value: usize, at: Span) -> Result<u32, Box<Diagnostic>> {
    let value = u32::try_from(value).map_err(|_| bad(at))?;
    if value >= BUILTIN_CONFLICT {
        return Err(bad(at));
    }
    Ok(value)
}
fn levels(n: u64) -> u64 {
    if n <= 1 {
        0
    } else {
        u64::from(64 - (n - 1).leading_zeros())
    }
}

impl IndexPlan {
    pub fn calculate(
        counts: Counts,
        outer: usize,
        fixed_scratch: usize,
        limits: IndexLimits,
        at: Span,
    ) -> Result<Self, Box<Diagnostic>> {
        let c = counts;
        // Complete arithmetic precedes every limit check and every allocation.
        let mut retained = outer as u64;
        for (count, bytes) in [
            (c.originals, 40),
            (c.functions, 12),
            (c.records, 28),
            (c.fields, 16),
            (c.modules, 60),
            (c.modules.checked_sub(1).ok_or_else(|| bad(at))?, 4),
            (c.imports, 40),
        ] {
            retained = add(retained, mul(count, bytes, at)?, at)?;
        }
        let scratch = add(
            add(
                add(
                    mul(c.originals.max(c.imports), 4, at)?,
                    mul(c.imports, 12, at)?,
                    at,
                )?,
                mul(c.modules, 4, at)?,
                at,
            )?,
            fixed_scratch as u64,
            at,
        )?;
        let visits = add(
            add(c.originals, c.fields, at)?,
            add(c.modules, c.imports, at)?,
            at,
        )?;
        let mut build_work = add(mul(visits, 16, at)?, 128, at)?;
        for (n, bytes) in [
            (c.originals, c.original_bytes),
            (c.imports, c.alias_bytes),
            (c.imports, c.path_weight),
        ] {
            build_work = add(
                build_work,
                mul(add(n, bytes, at)?, add(levels(n), 1, at)?, at)?,
                at,
            )?;
        }
        build_work = add(
            build_work,
            add(mul(c.originals, 2, at)?, mul(c.original_bytes, 4, at)?, at)?,
            at,
        )?;
        let limits = limits.lowered();
        if retained > limits.retained {
            return Err(resource(
                "declaration index retained byte limit exceeded",
                at,
            ));
        }
        if scratch > limits.scratch {
            return Err(resource(
                "declaration index scratch byte limit exceeded",
                at,
            ));
        }
        if build_work > limits.work {
            return Err(resource(
                "declaration index mandatory build work limit exceeded",
                at,
            ));
        }
        Ok(Self {
            counts,
            retained,
            scratch,
            build_work,
        })
    }
}

pub(super) fn allocate<T: Clone>(
    n: usize,
    value: T,
    allocator: &mut Allocator,
    name: &'static str,
    at: Span,
    work: &WorkMeter,
) -> Result<Vec<T>, Box<Diagnostic>> {
    n.checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| overflow(at))?;
    work.debit(n as u64, at, "initialize admitted rows")?;
    let mut result = Vec::new();
    allocator
        .vector(&mut result, n, name)
        .map_err(|e| match e {
            ReserveFailure::Overflow => overflow(at),
            ReserveFailure::Allocation => resource("declaration index allocation failed", at),
        })?;
    result.resize(n, value);
    Ok(result)
}

pub(super) fn compare_bytes(
    a: &str,
    b: &str,
    work: &WorkMeter,
    at: Span,
) -> Result<Ordering, Box<Diagnostic>> {
    work.debit(1, at, "comparison")?;
    for (left, right) in a.bytes().zip(b.bytes()) {
        work.debit(1, at, "compared byte")?;
        let order = left.cmp(&right);
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(a.len().cmp(&b.len()))
}

/// Stable, bounded, allocation-free merge after admission. Every emitted index
/// is copied exactly once per level; equality takes the left lexical row.
pub(super) fn merge_sort(
    order: &mut [u32],
    scratch: &mut [u32],
    mut compare: impl FnMut(u32, u32) -> Result<Ordering, Box<Diagnostic>>,
    work: &WorkMeter,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    if scratch.len() < order.len() {
        return Err(bad(at));
    }
    let mut width = 1usize;
    while width < order.len() {
        let mut start = 0;
        while start < order.len() {
            let middle = start.saturating_add(width).min(order.len());
            let end = middle.saturating_add(width).min(order.len());
            let (mut left, mut right) = (start, middle);
            for (offset, slot) in scratch[start..end].iter_mut().enumerate() {
                if left < middle
                    && (right >= end || compare(order[left], order[right])? != Ordering::Greater)
                {
                    *slot = order[left];
                    left += 1;
                } else {
                    *slot = order[right];
                    right += 1;
                }
                #[cfg(test)]
                work.observe(Observation::SortEmit {
                    width,
                    position: start + offset,
                    original: *slot,
                });
                #[cfg(not(test))]
                let _ = (offset, work);
            }
            if left != middle || right != end {
                return Err(bad(at));
            }
            start = end;
        }
        order.copy_from_slice(&scratch[..order.len()]);
        width = width.checked_mul(2).ok_or_else(|| overflow(at))?;
    }
    Ok(())
}
