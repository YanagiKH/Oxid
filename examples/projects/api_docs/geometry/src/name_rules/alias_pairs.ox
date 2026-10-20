use units::meters as Span;
use units::Meters as Span;

/// Only `Span` the type needs a spelling guide.
pub fn widen(m: Span) -> Span {
    return Span(m.value + 1);
}
