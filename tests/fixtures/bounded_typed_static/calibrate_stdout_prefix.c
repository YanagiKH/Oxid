#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
int main(int argc, char **argv) {
    if (argc != 2) return 1;
    int other = open(argv[1], O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (other < 3) return 2;
    if (write(other, "side-before\n", 12) != 12) return 3;
    if (write(1, "abcdefg", 7) != 3) return 4;
    errno = 0;
    if (write(1, "z", 1) != -1 || errno != EIO) return 5;
    if (write(2, "diagnostic\n", 11) != 11) return 6;
    if (write(other, "side-after\n", 11) != 11) return 7;
    return close(other) != 0;
}
