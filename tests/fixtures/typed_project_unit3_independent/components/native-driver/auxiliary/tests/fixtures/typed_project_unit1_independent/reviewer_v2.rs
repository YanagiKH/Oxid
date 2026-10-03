#[test]
fn heldout_scan_caps_and_checked_arithmetic_precede_iterator_error() {
    use std::{ffi::OsString,io};
    let at=Span{file:SourceFileId(0),start:4,end:5};
    let err=||Err::<OsString,_>(io::Error::other("independent deterministic I/O failure"));
    let mut l=ProjectLimits::default();l.directory_entries=0;
    let d=filesystem::scan_entries([err()],"a.ox",l,&mut SourceUsage::default(),at).unwrap_err();
    assert_eq!((d.code,d.stage,d.message.as_str()),("E0400","source-project","module directory scan budget exceeded"));
    l.directory_entries=1;
    let d=filesystem::scan_entries([err()],"a.ox",l,&mut SourceUsage::default(),at).unwrap_err();assert_eq!((d.code,d.stage),("E0002","source"));
    let mut u=SourceUsage{directory_entries:usize::MAX,..SourceUsage::default()};
    let d=filesystem::scan_entries([err()],"a.ox",l,&mut u,at).unwrap_err();assert_eq!(d.message,"project source count overflow");
    let d=filesystem::scan_entries([Ok(OsString::from("a.ox")),err()],"a.ox",l,&mut SourceUsage::default(),at).unwrap_err();assert_eq!(d.message,"module directory scan budget exceeded");
    let mut u=SourceUsage{directory_name_units:usize::MAX,..SourceUsage::default()};l.directory_entries=0;
    let d=filesystem::scan_entries([Ok(OsString::from("a"))],"a",l,&mut u,at).unwrap_err();assert_eq!(d.message,"project source count overflow");
}
#[test]
fn heldout_scan_is_complete_bounded_and_order_independent() {
    use std::ffi::OsString;
    let at=Span{file:SourceFileId(0),start:4,end:5};
    for values in [["a.ox","A.ox","z"],["z","a.ox","A.ox"],["A.ox","z","a.ox"]] {
        let mut l=ProjectLimits::default();l.directory_entries=2;
        let count=std::cell::Cell::new(0);
        let entries=values.into_iter().map(|s|{count.set(count.get()+1);Ok(OsString::from(s))});
        let d=filesystem::scan_entries(entries,"a.ox",l,&mut SourceUsage::default(),at).unwrap_err();assert_eq!(d.message,"module directory scan budget exceeded");assert_eq!(count.get(),3);
    }
    for (values,expected) in [(vec!["z","a.ox","A.ox"],(true,true)),(vec!["z","A.ox"],(false,true)),(vec!["z"],(false,false))] {
        let got=filesystem::scan_entries(values.into_iter().map(|x|Ok(OsString::from(x))),"a.ox",ProjectLimits::default(),&mut SourceUsage::default(),at).unwrap();assert_eq!(got,expected);
    }
    let count=std::cell::Cell::new(0);let mut l=ProjectLimits::default();l.directory_entries=1;
    let entries=["a.ox","x","never"].into_iter().map(|s|{count.set(count.get()+1);Ok(OsString::from(s))});
    let d=filesystem::scan_entries(entries,"a.ox",l,&mut SourceUsage::default(),at).unwrap_err();assert_eq!(d.message,"module directory scan budget exceeded");assert_eq!(count.get(),2);
}
mod compatibility_schedule {
    include!("scalar_schedule_heldout.rs");
}
