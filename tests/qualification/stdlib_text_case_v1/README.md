# ASCII text case storage successor

The language feature is `stdlib/text.ox` ASCII upper/lower conversion. The exact
accepted historical text body lives in `archive/stdlib/text.ox.data`. The immutable
2724-body descriptor, source seal, and 376-source compiler roster are unchanged.

`derive.py` adds precisely one fixed relocation to the accepted PR65 predecessor.
The whole stdlib_trim_storage_v1 namespace remains byte-identical.

`baseline.py` authenticates a finite PR65 expected-output fixture against raw Git
commit b416d460af657a53ca9419aefccbb2d29adafa97 and its tree. It executes the exact
old producer with private import aliases after checking its twelve extra physical
inputs; the existing PR64 reader checks the remaining fixed inputs. The 27 old
outputs are checked against PR65 storage, never against new active output. Only
changed expected outputs are archived; unchanged bodies and the named old input
inventory are read in place. This is not a whole-checkout snapshot or a sandbox.

`activate.py --check` reproduces the explicit current output set without writes.
`--write` writes only that set. It emits new named parser/lifecycle/hosted proofs
and updates current admissions. No old proof or source qualification is relabeled.
`adapters.py` retains prior test assertions and changes only generation-specific
expected reads and active helper selection; current admission runs against real
current files. New tests cover text relocation, private imports and mixed proofs.

The fixed fixture has 40 rows: 27 expected outputs, twelve producer inputs,
and the unchanged source reader. Eighteen changed output bodies are archived.
`review_inputs.py --check` independently reproduces its metadata, raw Git tree
proof, archived bodies and historical text from the accepted commit (20 outputs).
It requires local Git objects and never fetches. Current activation emits 26
outputs; the new active inventory has 1127 rows and 19 closed roots.
