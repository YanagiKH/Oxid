#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>
/* Evidence-only narrow adaptation of output-process-fault-controls/faults.c.
 * Inject a returned SIG_ERR, never deliver or simulate an actual signal.
 * Limit selection to this exact executable, exact stdout/stderr objects,
 * SIGPIPE+SIG_IGN and a calibrated matching-call ordinal. Trace F=forwarded,
 * X=injected failure to one exact inherited evidence file descriptor. */
typedef void (*handler)(int);
static unsigned matches;
static int identity(const struct stat *s,const char *dev,const char *ino) {
    const char *d=getenv(dev),*i=getenv(ino);
    return d&&i&&(uintmax_t)s->st_dev==strtoull(d,0,10)&&(uintmax_t)s->st_ino==strtoull(i,0,10);
}
static int fd_identity(int fd,const char *dev,const char *ino) {
    struct stat s;return fstat(fd,&s)==0&&identity(&s,dev,ino);
}
static int selected(void) {
    struct stat s;
    return stat("/proc/self/exe",&s)==0&&identity(&s,"OXID_SETUP_EXE_DEV","OXID_SETUP_EXE_INO")&&
        fd_identity(1,"OXID_SETUP_STDOUT_DEV","OXID_SETUP_STDOUT_INO")&&
        fd_identity(2,"OXID_SETUP_STDERR_DEV","OXID_SETUP_STDERR_INO");
}
static void trace(char value) {
    const char *f=getenv("OXID_SETUP_TRACE_FD");
    if(!f)return;
    int fd=atoi(f);
    if(fd>2&&fd_identity(fd,"OXID_SETUP_TRACE_DEV","OXID_SETUP_TRACE_INO")) {
        ssize_t (*actual)(int,const void*,size_t)=dlsym(RTLD_NEXT,"write");
        if(actual)(void)actual(fd,&value,1);
    }
}
static handler install(const char *symbol,int number,handler value) {
    handler (*actual)(int,handler)=dlsym(RTLD_NEXT,symbol);
    if(!actual){errno=EINVAL;return SIG_ERR;}
    if(number==SIGPIPE&&value==SIG_IGN&&selected()) {
        const char *ordinal=getenv("OXID_SETUP_FAIL_ORDINAL");
        ++matches;
        if(ordinal&&matches==strtoul(ordinal,0,10)) {
            trace('X');errno=EINVAL;return SIG_ERR;
        }
        trace('F');
    }
    return actual(number,value);
}
handler signal(int number,handler value){return install("signal",number,value);}
handler __sysv_signal(int number,handler value){return install("__sysv_signal",number,value);}
