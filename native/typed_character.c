/* Encode a checked Unicode scalar directly into the caller-owned output. */
int __oxid_print_char(uint32_t value) {
    unsigned char bytes[5];
    size_t length;
    if (value <= 0x7f) {
        bytes[0] = (unsigned char)value;
        length = 1;
    } else if (value <= 0x7ff) {
        bytes[0] = (unsigned char)(0xc0 | (value >> 6));
        bytes[1] = (unsigned char)(0x80 | (value & 0x3f));
        length = 2;
    } else if (value <= 0xffff && !(value >= 0xd800 && value <= 0xdfff)) {
        bytes[0] = (unsigned char)(0xe0 | (value >> 12));
        bytes[1] = (unsigned char)(0x80 | ((value >> 6) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | (value & 0x3f));
        length = 3;
    } else if (value >= 0x10000 && value <= 0x10ffff) {
        bytes[0] = (unsigned char)(0xf0 | (value >> 18));
        bytes[1] = (unsigned char)(0x80 | ((value >> 12) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | ((value >> 6) & 0x3f));
        bytes[3] = (unsigned char)(0x80 | (value & 0x3f));
        length = 4;
    } else {
        return 74;
    }
    bytes[length++] = '\n';
    return output(STDOUT_FILENO, (const char *)bytes, length);
}
