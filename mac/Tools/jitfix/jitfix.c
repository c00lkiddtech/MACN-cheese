// Works around JDK-8326663: HotSpot SIGBUS in CodeHeap::allocate on Apple
// Silicon when SIP/AMFI are disabled (MAP_JIT pages left read-execute).
#include <pthread.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>
static struct sigaction prev;
static void handler(int sig, siginfo_t *info, void *ctx) {
    static __thread int depth = 0;
    if (depth == 0 && info && info->si_addr) {
        depth++;
        pthread_jit_write_protect_np(0);
        long pg = getpagesize();
        void *page = (void *)((unsigned long)info->si_addr & ~(unsigned long)(pg - 1));
        (void)mprotect(page, pg, PROT_READ | PROT_WRITE | PROT_EXEC); depth--; return;
    }
    if (prev.sa_flags & SA_SIGINFO) { if (prev.sa_sigaction) prev.sa_sigaction(sig, info, ctx); }
    else if (prev.sa_handler != SIG_DFL && prev.sa_handler != SIG_IGN) prev.sa_handler(sig);
    else { signal(sig, SIG_DFL); raise(sig); }
}
__attribute__((constructor)) static void init(void) {
    pthread_jit_write_protect_np(0);
    stack_t ss; ss.ss_size = 1 << 16; ss.ss_sp = malloc(ss.ss_size); ss.ss_flags = 0; sigaltstack(&ss, NULL);
    struct sigaction sa; memset(&sa, 0, sizeof sa);
    sa.sa_sigaction = handler; sa.sa_flags = SA_SIGINFO | SA_ONSTACK; sigemptyset(&sa.sa_mask);
    sigaction(SIGBUS, &sa, &prev);
}
