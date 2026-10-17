//! Closed scalar observation versions. V1/V2 retain their original byte wire;
//! V3 uses checked wide endpoints and a separately selected compact transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Protocol {
    V1,
    V2,
    V3,
}
impl Protocol {
    pub(super) fn source_max(self) -> usize {
        match self {
            Self::V1 => 128,
            Self::V2 => 255,
            Self::V3 => 511,
        }
    }
    pub(super) fn digit(self) -> u8 {
        match self {
            Self::V1 => b'1',
            Self::V2 => b'2',
            Self::V3 => b'3',
        }
    }
    pub(super) fn opa_bytes(self) -> usize {
        match self {
            Self::V1 | Self::V2 => 1559,
            Self::V3 => 1304,
        }
    }
    pub(super) fn success_bytes(self) -> usize {
        match self {
            Self::V1 | Self::V2 => 2607,
            Self::V3 => 2356,
        }
    }
    pub(super) fn diagnostic_bytes(self) -> usize {
        match self {
            Self::V1 | Self::V2 => 1575,
            Self::V3 => 1324,
        }
    }
    pub(super) fn from_opa(bytes: &[u8]) -> Option<Self> {
        match bytes.get(..4)? {
            b"OPA1" => Some(Self::V1),
            b"OPA2" => Some(Self::V2),
            b"OPA3" => Some(Self::V3),
            _ => None,
        }
    }
}
