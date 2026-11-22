pub struct Generator { state: u32 }
pub enum Draw { Value(u32), EmptyRange }

pub fn seeded(seed: u32) -> Generator {
    return Generator { state: seed };
}

pub fn next_nonzero(generator: &mut Generator) -> u32 {
    let mut bits = generator.state;
    if bits == 0u32 { bits = 1831565813u32; }
    bits = bits ^ (bits << 13);
    bits = bits ^ (bits >> 17);
    bits = bits ^ (bits << 5);
    generator.state = bits;
    return bits;
}

pub fn below(generator: &mut Generator, upper: u32) -> Draw {
    if upper == 0u32 { return Draw::EmptyRange; }
    let period = 4294967295u32;
    let limit = period - period % upper;
    let mut candidate = next_nonzero(&mut *generator) - 1u32;
    while candidate >= limit {
        candidate = next_nonzero(&mut *generator) - 1u32;
    }
    return Draw::Value(candidate % upper);
}
