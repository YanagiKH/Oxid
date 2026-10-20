use kernel::scale::double;

pub fn square_area(side: i32) -> i32 {
    return side * side;
}

pub fn doubled_area(side: i32) -> i32 {
    return double(side * side);
}
