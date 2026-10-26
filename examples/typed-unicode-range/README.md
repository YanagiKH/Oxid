# Print a Unicode range

Run `oxid run -- 65 91` in this directory to print `ABCDEFGHIJKLMNOPQRSTUVWXYZ`.
The first argument is the starting scalar value; the second is the exclusive
end, from zero through 1114112. Surrogate values are skipped. No newline is added.

The example appends `char` values to owned UTF-8 text, reads them back with
`char_at`, and writes the encoded bytes. Invalid arguments exit with status 64;
allocation failures use 70 and output errors use 74.
