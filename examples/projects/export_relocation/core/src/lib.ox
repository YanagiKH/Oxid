/// Shared kernel: one package in the export even though two packages use it.
pub mod scale;

pub struct Seed {
    pub value: i32,
}

pub fn seed() -> Seed {
    return Seed { value: 5 };
}
