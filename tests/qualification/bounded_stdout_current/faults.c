#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

typedef void (*handler)(int);
static unsigned output_attempts, diagnostic_attempts;
static int selected(int fd, const char *dev_name, const char *ino_name) {
    const char *dev=getenv(dev_name), *ino=getenv(ino_name);
    struct stat st;
    return dev && ino && fstat(fd,&st)==0 &&
        (uintmax_t)st.st_dev==strtoull(dev,0,10) &&
        (uintmax_t)st.st_ino==strtoull(ino,0,10);
}
static int stdout_selected(void) {
    return selected(1,"OXID_FAULT_STDOUT_DEV","OXID_FAULT_STDOUT_INO");
}
ssize_t write(int fd,const void *bytes,size_t count) {
    ssize_t (*actual)(int,const void*,size_t)=dlsym(RTLD_NEXT,"write");
    if(!actual){errno=EIO;return -1;}
    const char *mode=getenv("OXID_OUTPUT_FAULT");
    if(mode && fd==1 && count==1 && stdout_selected()) {
        if(strcmp(mode,"two-eintr")==0 && output_attempts++<2){errno=EINTR;return -1;}
        if(strcmp(mode,"zero")==0)return 0;
    }
    if(mode && fd==2 && count && selected(2,"OXID_FAULT_STDERR_DEV","OXID_FAULT_STDERR_INO")) {
        if(strcmp(mode,"diagnostic-short-eintr")==0) {
            unsigned n=diagnostic_attempts++;
            if(n==0){errno=EINTR;return -1;}
            if(n==1)return actual(fd,bytes,1);
        }
        if(strcmp(mode,"diagnostic-short-error")==0) {
            if(diagnostic_attempts++==0)return actual(fd,bytes,1);
            errno=EIO;return -1;
        }
    }
    return actual(fd,bytes,count);
}
static handler install(const char *symbol,int number,handler value) {
    const char *mode=getenv("OXID_OUTPUT_FAULT");
    if(number==SIGPIPE && mode && strcmp(mode,"setup-failure")==0 && stdout_selected()) {
        errno=EINVAL;return SIG_ERR;
    }
    handler (*actual)(int,handler)=dlsym(RTLD_NEXT,symbol);
    if(!actual){errno=EINVAL;return SIG_ERR;}
    return actual(number,value);
}
handler signal(int number,handler value){return install("signal",number,value);}
handler __sysv_signal(int number,handler value){return install("__sysv_signal",number,value);}
