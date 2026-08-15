/* The native backend's runtime.
 *
 * Compiled and linked into every native binary by the `cc` invocation that already links the
 * object file. The generated LLVM IR declares these and calls them; nothing here knows about
 * toylang's types beyond what its signatures say.
 *
 * Nothing frees. Prototype 1.5 leaks deliberately: choosing between refcounting and tracing
 * belongs with the mutation model, and a half-built refcount would be worse than an honest
 * leak in a program that runs once and exits. Keeping every allocation in this file keeps that
 * decision in one visible place.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* A toylang Str: bytes and a length, never null-terminated by contract. The bytes a literal
 * points at happen to carry a trailing NUL so that a debugger can print them, but len excludes
 * it and no code here reads past len. */
typedef struct {
    const char *ptr;
    int64_t len;
} tl_str;

static void *tl_alloc(size_t n) {
    void *p = malloc(n);
    if (p == NULL) {
        const char *msg = "toylang: out of memory\n";
        write(2, msg, strlen(msg));
        exit(1);
    }
    return p;
}

static tl_str *tl_str_new(char *bytes, int64_t len) {
    tl_str *s = tl_alloc(sizeof(tl_str));
    s->ptr = bytes;
    s->len = len;
    return s;
}

tl_str *tl_concat(const tl_str *a, const tl_str *b) {
    char *bytes = tl_alloc((size_t)(a->len + b->len));
    memcpy(bytes, a->ptr, (size_t)a->len);
    memcpy(bytes + a->len, b->ptr, (size_t)b->len);
    return tl_str_new(bytes, a->len + b->len);
}

tl_str *tl_int_to_str(int64_t n) {
    /* -9223372036854775808 is 20 characters plus a terminator. */
    char buf[24];
    int len = snprintf(buf, sizeof buf, "%lld", (long long)n);
    char *bytes = tl_alloc((size_t)len);
    memcpy(bytes, buf, (size_t)len);
    return tl_str_new(bytes, len);
}

int64_t tl_str_eq(const tl_str *a, const tl_str *b) {
    if (a->len != b->len) {
        return 0;
    }
    return memcmp(a->ptr, b->ptr, (size_t)a->len) == 0;
}

/* Byte order, which is what Lua does. JavaScript compares UTF-16 code units, so the three
 * backends agree on ASCII and are not guaranteed to beyond it. */
int64_t tl_str_cmp(const tl_str *a, const tl_str *b) {
    int64_t shared = a->len < b->len ? a->len : b->len;
    int diff = memcmp(a->ptr, b->ptr, (size_t)shared);
    if (diff != 0) {
        return diff < 0 ? -1 : 1;
    }
    if (a->len == b->len) {
        return 0;
    }
    return a->len < b->len ? -1 : 1;
}

/* One write for the payload and one for the newline, rather than copying to join them. */
void tl_print(const tl_str *s) {
    if (s->len > 0) {
        (void)!write(1, s->ptr, (size_t)s->len);
    }
    (void)!write(1, "\n", 1);
}
