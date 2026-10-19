pub struct Header {
    signature: [u8; 8],
    depth: u8,
    color: u8,
    width: i32,
    height: i32
}
pub enum Parsed {
    Ready(Header),
    Invalid(i32)
}
fn byte(value: i32) -> u8 {
    return value.to_u8_checked();
}
fn dimension(input: &[i32], start: i32) -> i32 {
    return input[start] * 16777216 + input[start + 1] * 65536 + input[start + 2] * 256 + input[start + 3];
}
pub fn parse(input: &[i32]) -> Parsed {
    if input.len() != 33 {
        return Parsed::Invalid(-1);
    }
    let mut i = 0;
    while i < input.len() {
        if input[i] < 0 || input[i] > 255 {
            return Parsed::Invalid(-2);
        }
        i = i + 1;
    }
    if input[0] != 137 || input[1] != 80 || input[2] != 78 || input[3] != 71 || input[4] != 13 || input[5] != 10 || input[6] != 26 || input[7] != 10 {
        return Parsed::Invalid(-3);
    }
    if input[8] != 0 || input[9] != 0 || input[10] != 0 || input[11] != 13 || input[12] != 73 || input[13] != 72 || input[14] != 68 || input[15] != 82 {
        return Parsed::Invalid(-4);
    }
    if input[16] > 127 || input[20] > 127 {
        return Parsed::Invalid(-5);
    }
    let w = dimension(&*input, 16);
    let h = dimension(&*input, 20);
    if w == 0 || h == 0 {
        return Parsed::Invalid(-5);
    }
    let header = Header {
        signature: [byte(input[0]), byte(input[1]), byte(input[2]), byte(input[3]), byte(input[4]), byte(input[5]), byte(input[6]), byte(input[7])],
        depth: byte(input[24]),
        color: byte(input[25]),
        width: w,
        height: h
    };
    return Parsed::Ready(header);
}
pub fn width(header: &Header) -> i32 {
    return header.width;
}
pub fn height(header: &Header) -> i32 {
    return header.height;
}
pub fn depth(header: &Header) -> i32 {
    let value = header.depth;
    return value.to_i32();
}
pub fn color(header: &Header) -> i32 {
    let value = header.color;
    return value.to_i32();
}
pub fn signature_sum(header: &Header) -> i32 {
    return sum(&*header.signature);
}
fn sum(bytes: &[u8]) -> i32 {
    let mut total = 0;
    let mut i = 0;
    while i < bytes.len() {
        let value = bytes[i];
        total = total + value.to_i32();
        i = i + 1;
    }
    return total;
}
