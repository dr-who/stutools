/* Native synchronous file backend. Linux keeps its full libaio engine. */
#include "native_io.h"
#include <spit-version.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <inttypes.h>
#include <math.h>
#include <errno.h>
#include <signal.h>
#include <sys/stat.h>

static volatile sig_atomic_t running = 1;
static void stop(int sig) { (void)sig; running = 0; }
static void fail(const char *message) { fprintf(stderr, "*error* %s\n", message); exit(1); }
static double number(const char *s, char **end) {
    errno = 0; double n = strtod(s, end);
    if (*end == s || errno || !isfinite(n) || n < 0) fail("invalid numeric argument");
    return n;
}
static uint64_t size_value(const char *s) {
    char *end; double n = number(s, &end), scale = 1e9;
    if (!strcmp(end,"B")) scale=1;
    else if (!strcmp(end,"KiB")) scale=1024;
    else if (!strcmp(end,"MiB")) scale=1048576;
    else if (!strcmp(end,"GiB")) scale=1073741824;
    else if (!strcmp(end,"kB")) scale=1000;
    else if (!strcmp(end,"MB")) scale=1e6;
    else if (!strcmp(end,"GB")) scale=1e9;
    else if (*end) fail("unknown size suffix; use B, KiB, MiB, GiB, kB, MB, or GB");
    if (n*scale > (double)INT64_MAX || n*scale < 1) fail("size out of range");
    return (uint64_t)(n*scale);
}
static uint64_t random64(uint64_t *state) {
    *state += UINT64_C(0x9e3779b97f4a7c15);
    uint64_t z = *state;
    z = (z ^ (z >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)) * UINT64_C(0x94d049bb133111eb);
    return z ^ (z >> 31);
}
static void pattern(unsigned char *p, size_t n, uint64_t pos, uint64_t seed) {
    uint64_t state = pos ^ seed;
    for (size_t i=0; i<n; i++) { if ((i & 7)==0) state=random64(&state); p[i]=(unsigned char)(state >> ((i & 7)*8)); }
}
static void usage(void) {
    printf("spit %s — native synchronous file backend\n", SPIT_VERSION);
    puts("Usage: spit -f FILE -G SIZE[-SIZE] -c JOB [-t SECONDS] [-v] [-R SEED] [--recreate]\n"
         "JOB: r/w/rw or p0..1; s0(random)/s1(sequential); kKiB or MMiB; q1; j1;\n"
         "     xPASSES, XREPETITIONS, PPOSITIONS, z (start at zero).\n"
         "-v verifies each completed write by reading it back. Existing files are preserved\n"
         "unless --recreate explicitly permits resizing a write target.\n"
         "This native backend uses buffered, synchronous regular-file I/O with one worker.\n"
         "Linux-only options, raw devices, queue depths >1 and multiple jobs are unsupported\n"
         "and rejected. Its throughput is not equivalent to Linux direct/AIO benchmarks.");
}
int main(int argc, char **argv) {
    const char *path=NULL, *job=NULL, *range=NULL;
    double seconds=10, read_probability=-1;
    uint64_t seed=1, block=4096, passes=0, positions=0, repeats=0;
    int verify=0, recreate=0, sequential=1, reads=0, writes=0;
    for (int i=1; i<argc; i++) {
        const char *a=argv[i];
        if (!strcmp(a,"-h") || !strcmp(a,"--help")) { usage(); return 0; }
        if (!strcmp(a,"--version")) { puts(SPIT_VERSION); return 0; }
        if (!strcmp(a,"-v")) { verify=1; continue; }
        if (!strcmp(a,"--recreate")) { recreate=1; continue; }
        if (strlen(a)!=2 || !strchr("fGctR",a[1]) || ++i>=argc) fail("unsupported option or missing argument; use -h");
        const char *v=argv[i]; char *end;
        switch(a[1]) {
            case 'f': if(path) fail("only one target supported"); path=v; break;
            case 'G': range=v; break;
            case 'c': if(job) fail("only one job supported"); job=v; break;
            case 't': seconds=number(v,&end); if(*end || seconds<=0) fail("duration must be positive"); break;
            case 'R': { double n=number(v,&end); if(*end || n>UINT32_MAX || floor(n)!=n) fail("invalid seed"); seed=(uint64_t)n; break; }
        }
    }
    if(!path || !job) { usage(); return 1; }
    for(const char *p=job; *p;) {
        char c=*p++; char *end=NULL; double n;
        if(c=='r') { reads++; continue; }
        if(c=='w') { writes++; continue; }
        if(c=='z') continue;
        if(!strchr("pskMqjxXP",c)) fail("unsupported native job option; use -h");
        n=number(p,&end); p=end;
        if(c=='p') { if(n>1) fail("read probability must be in [0,1]"); read_probability=n; }
        else if(c=='s') { if(n!=0 && n!=1) fail("native sequentialness must be 0 or 1"); sequential=(int)n; }
        else if(c=='k' || c=='M') {
            n *= c=='k' ? 1024 : 1048576;
            if(n<512 || n>67108864 || floor(n)!=n || ((uint64_t)n%512)) fail("block size must be 512-byte aligned and at most 64 MiB");
            block=(uint64_t)n;
        } else {
            if(n<1 || n>UINT32_MAX || floor(n)!=n) fail("invalid job count");
            if(c=='q' || c=='j') { if(n!=1) fail("native backend requires q1 and j1"); }
            if(c=='x') passes=(uint64_t)n;
            if(c=='X') repeats=(uint64_t)n;
            if(c=='P') positions=(uint64_t)n;
        }
    }
    if(read_probability<0) read_probability=reads+writes ? (double)reads/(reads+writes) : 0.5;
    int writing=read_probability<1;
    uint64_t lower=0, upper=0;
    if(range) {
        char *copy=malloc(strlen(range)+1); if(!copy) fail("out of memory"); strcpy(copy,range);
        char *dash=strchr(copy,'-');
        if(dash) { *dash=0; lower=!strcmp(copy,"0") ? 0 : size_value(copy); upper=size_value(dash+1); }
        else upper=size_value(copy);
        free(copy);
    }
    native_file f; int exists=1;
    if(native_open(&f,path,writing,0)) {
        struct stat st;
        if(!stat(path,&st) || !writing) { native_error("open"); return 1; }
        if(native_open(&f,path,1,1)) { native_error("create"); return 1; }
        exists=0;
    }
    uint64_t actual=0;
    if(native_size(&f,&actual)) { native_error("file size"); native_close(&f); return 1; }
    if(!upper) upper=exists ? actual : UINT64_C(67108864);
    if(lower>=upper || block>upper-lower) { native_close(&f); fail("empty range or block larger than range"); }
    if(exists && upper>actual && !(writing && recreate)) { native_close(&f); fail("range exceeds file; resizing write jobs require --recreate"); }
    if(!exists || (writing && recreate && actual!=upper)) {
        if(native_resize(&f,upper)) { native_error("resize"); native_close(&f); return 1; }
    }
    uint64_t blocks=(upper-lower)/block;
    if(positions>blocks) { native_close(&f); fail("position count exceeds range"); }
    uint64_t count=positions ? positions : blocks;
    if(passes && repeats) { native_close(&f); fail("choose x or X, not both"); }
    uint64_t rounds=passes ? passes : repeats;
    if(rounds && count>UINT64_MAX/rounds) { native_close(&f); fail("operation count overflow"); }
    uint64_t limit=rounds ? count*rounds : 0;
    unsigned char *buf=malloc((size_t)block), *check=verify ? malloc((size_t)block) : NULL;
    if(!buf || (verify&&!check)) { native_close(&f); free(buf); free(check); fail("out of memory"); }
    signal(SIGINT,stop); signal(SIGTERM,stop);
    fprintf(stderr,"spit %s: native buffered synchronous I/O, q1 j1\n",SPIT_VERSION);
    uint64_t state=seed, done=0, read_bytes=0, write_bytes=0; int error=0;
    double start=native_time();
    while(running && (limit ? done<limit : native_time()-start<seconds)) {
        uint64_t index=sequential ? done%count : random64(&state)%count;
        uint64_t offset=lower+index*block;
        int write_op=(double)(random64(&state)>>11)/9007199254740992.0>=read_probability;
        if(write_op) pattern(buf,(size_t)block,offset,seed);
        if(native_io(&f,buf,(size_t)block,offset,write_op)) { native_error("I/O"); error=1; break; }
        if(write_op) write_bytes+=block; else read_bytes+=block;
        if(write_op && verify) {
            if(native_io(&f,check,(size_t)block,offset,0) || memcmp(buf,check,(size_t)block)) {
                fprintf(stderr,"verification failed at byte %" PRIu64 "\n",offset); error=1; break;
            }
        }
        done++;
    }
    if(writing && native_sync(&f)) { native_error("flush"); error=1; }
    double elapsed=native_time()-start;
    printf("operations=%" PRIu64 " read_bytes=%" PRIu64 " write_bytes=%" PRIu64 " seconds=%.6f\n",done,read_bytes,write_bytes,elapsed);
    if(verify && !error) puts("verification passed");
    native_close(&f); free(buf); free(check);
    return error || !running ? 1 : 0;
}
