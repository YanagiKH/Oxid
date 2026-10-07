# Denied HIR import observation

`rich-source.txt` is the exact 113-byte input to the existing bounded Oxid parser.
Its unchanged OPA1 output and original source bytes were framed as AST1 for the
existing Oxid static consumer. `rich-success.bin` is that consumer's actual
2,607-byte successful observation, with 29 active rows and item head 1.
`origin.json` records the input, executable and output identities; its short
command labels identify the two separate producer processes.

This fixture catches row-count/item-head confusion without reconstructing a
success artifact in the host. It is untrusted input to a permanently denied
compiler precursor, not a typed-program or executable-authority token. The
compiler still must reject every import request at this stage. Its framing tests
do not establish complete source/AST or semantic validation.

`public-source.txt` and `public-success.bin` retain the equivalent 117-byte
public-function request and its actual producer observation. The test loads this
source through genuine root-only ProjectSources and rejects an original-adapter
substitution before framing. `public-origin.json` preserves its identities.
