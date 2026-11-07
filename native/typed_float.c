/* Decimal output uses the same allocation-free core formatter as the reference
 * implementation. The caller owns this fixed buffer and writes through the
 * existing EINTR/broken-pipe adapter. */
extern size_t oxid_format_float(uint64_t, uint32_t, unsigned char *, size_t);
int __oxid_print_float_bits(uint64_t bits, int32_t width) {
    unsigned char buffer[513];
    size_t length = oxid_format_float(bits, (uint32_t)width, buffer, sizeof buffer - 1);
    if (length == 0 || length >= sizeof buffer) return 74;
    buffer[length++] = '\n';
    return output(STDOUT_FILENO, (const char *)buffer, length);
}
