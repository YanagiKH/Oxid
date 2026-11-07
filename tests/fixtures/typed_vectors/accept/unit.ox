use alloc::vec::Vec;
use alloc::vec::GrowStatus;
use alloc::vec::new;
use alloc::vec::len;
use alloc::vec::capacity;
use alloc::vec::try_reserve;
use alloc::vec::try_push;
use alloc::vec::try_extend;
use alloc::vec::try_resize;
use alloc::vec::truncate;
use alloc::vec::take;
fn growth_ok(status:GrowStatus)->bool{
match status{
GrowStatus::Ok=>{return true;},
GrowStatus::CapacityOverflow=>{return false;},
GrowStatus::AllocationFailed=>{return false;},
GrowStatus::MemoryLimit=>{return false;},
}}
fn read_second<T:Copy>(values:&[T])->T{return values[1];}
fn identity<T:Copy>(values:Vec<T>)->Vec<T>{return values;}
fn main()->i32{let a=();let b=();let mut values=new::<()>();if len(&values)!=0||capacity(&values)!=0{return 1;}if !growth_ok(try_push(&mut values,a)){return 2;}if !growth_ok(try_push(&mut values,b)){return 3;}if len(&values)!=2{return 4;}if !growth_ok(try_reserve(&mut values,13)){return 5;}if capacity(&values)<15||len(&values)!=2{return 6;}let source=[a,b];if !growth_ok(try_extend(&mut values,&source)){return 7;}if !growth_ok(try_resize(&mut values,17,b)){return 8;}let first=values[0];let second=read_second(&values);if len(&values)!=17{return 13;}let grown=values[16];let saved_capacity=capacity(&values);truncate(&mut values,3);if len(&values)!=3||capacity(&values)!=saved_capacity{return 10;}let moved=take(&mut values);if len(&values)!=0||capacity(&values)!=0{return 11;}let final_values=identity(moved);if len(&final_values)!=3{return 12;}return 42;}
