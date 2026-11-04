fn main() -> i32 {
    let mut answer = 0;
    { let value: i32 = 40; answer = answer + value; }
    { let value = 2; answer = answer + value; }
    return answer;
}
