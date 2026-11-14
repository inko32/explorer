// No-op stubs for libsysprof-capture symbols that Arch's static
// libglib-2.0.a references (its .so twin is built without sysprof and
// never resolves them). Tracing is never active in this app, so a stub
// with the right ABI is sufficient to satisfy the static link.
#include <stdint.h>
#include <stdarg.h>

int64_t sysprof_clock(void) { return 0; }

void sysprof_collector_request_counters(unsigned int n_counters) { (void)n_counters; }

void sysprof_collector_define_counters(const void *counters, unsigned int n_counters) {
    (void)counters; (void)n_counters;
}

void sysprof_collector_set_counters(const uint64_t *counters) { (void)counters; }

void sysprof_collector_mark_vprintf(const char *group, const char *mark, va_list args) {
    (void)group; (void)mark; (void)args;
}
