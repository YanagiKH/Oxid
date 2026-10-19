use math::answer;
use math::model::make;
use math::model::value;

fn main()->i32 { let r=make(); return value(&r)+answer(); }
fn test_record()->bool { let r=make(); return value(&r)==7; }
fn test_scalar()->bool { return answer()==7; }
