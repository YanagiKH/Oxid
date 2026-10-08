//! Closed scalar observation versions. Only source capacity changes in v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Protocol {
    V1,
    V2,
}
impl Protocol {
    pub(super) fn source_max(self) -> usize {
        match self {
            Self::V1 => 128,
            Self::V2 => 255,
        }
    }
    pub(super) fn digit(self) -> u8 {
        match self {
            Self::V1 => b'1',
            Self::V2 => b'2',
        }
    }
    pub(super) fn from_opa(bytes: &[u8]) -> Option<Self> {
        match bytes.get(..4)? {
            b"OPA1" => Some(Self::V1),
            b"OPA2" => Some(Self::V2),
            _ => None,
        }
    }
}
