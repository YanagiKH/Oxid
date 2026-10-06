#include <errno.h>
#include <unistd.h>
int main(void) {
    char unrelated='C', byte='Z';
    if(write(2,&unrelated,1)!=1)return 91;
    if(write(1,&byte,1)!=-1 || errno!=EINTR)return 92;
    if(write(1,&byte,1)!=-1 || errno!=EINTR)return 93;
    if(write(1,&byte,1)!=1)return 94;
    return 0;
}
