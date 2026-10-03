# One source-only mutation control correction

Review candidate, created before any execution of this mutation.

The original request raw-wrong_scalar_result names control-span-variants-scalar.
That frozen source's only call is noop(n) returning unit. Its independent model
also records result_type=(). The prescribed opposite scalar result type applies
to i32/bool and cannot select this call. No result of this mutation was observed.

This supplement selects the existing frozen scalar-root_child-absolute-entry0-i32
control, whose only call invokes the original left helper returning i32. The
unchanged mutation changes that result slot to bool. Its expected authority
remains raw-verifier rejection, with no verified witness and internal E0500.

build.py reads only original frozen source/model/request files and reproduces
the complete requests/expectations plus a source proof. Exactly one positive
control field differs; all 114 IDs, mutation actions and expected classifications
remain unchanged. Original files and the discovery history are retained.

This does not add a source fixture, compiler execution or broaden an expected
alternative. The request projection may be given to an observer only after the
source-only correction is independently reviewed.
