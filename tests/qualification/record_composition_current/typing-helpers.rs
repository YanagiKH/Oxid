fn historical_field_scalar(ty: ValueTy) -> Result<&'static str, String> {
    match ty {
        ValueTy::Scalar(ty) => Ok(scalar(ty)),
        ValueTy::Owned(_) => Err("incomplete-observation: historical scalar record field required".into()),
    }
}
fn historical_referent(referent: BorrowedTy) -> Result<AggregateTy, fmt::Error> {
    match referent {
        BorrowedTy::Exact(aggregate) => Ok(aggregate),
        BorrowedTy::ScalarSlice(_) => Err(fmt::Error),
    }
}
