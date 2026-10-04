pub struct Stats { count: i32, sum: i32 }
pub fn create() -> Stats { return Stats { count: 0, sum: 0 }; }
pub fn record(s: &mut Stats, value: i32) -> () {
    s.count = s.count + 1;
    s.sum = s.sum + value;
    return;
}
pub fn count(s: &Stats) -> i32 { return s.count; }
pub fn sum(s: &Stats) -> i32 { return s.sum; }
