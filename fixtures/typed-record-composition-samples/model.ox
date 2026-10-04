pub struct Batch {
    pub meta: Meta,
    pub samples: [i32; 3],
}

pub struct Meta {
    pub completed: i32,
}
