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

/* A Vec of scalars: a length and one column of elements.
 *
 * The element width is passed in rather than baked in, so one set of functions serves Vec<Int>
 * (8-byte payloads) and Vec<Str> (8-byte pointers) alike. Step 6b generalises this to several
 * columns for a Vec of records; the layout is struct-of-arrays, so adding fields adds columns
 * rather than widening an element.
 */
typedef struct {
    int64_t len;
    void *col;
} tl_vec;

tl_vec *tl_vec_new(int64_t len, int64_t width) {
    tl_vec *v = tl_alloc(sizeof(tl_vec));
    v->len = len;
    v->col = len > 0 ? tl_alloc((size_t)(len * width)) : NULL;
    return v;
}

int64_t tl_vec_len(const tl_vec *v) {
    return v->len;
}

/* Elements are read and written as raw 8-byte slots. Every scalar toylang has fits one: an Int
 * is an i64 and a Str is a pointer. A narrower type would need a width here too. */
int64_t tl_vec_get(const tl_vec *v, int64_t i) {
    return ((const int64_t *)v->col)[i];
}

void tl_vec_set(tl_vec *v, int64_t i, int64_t value) {
    ((int64_t *)v->col)[i]  = value;
}

/* select in two passes: count survivors, then fill. Two passes over a mask beats growing an
 * array, and it is the shape that stays correct when the columns multiply in step 6b. */
tl_vec *tl_vec_from_mask(const tl_vec *src, const int8_t *keep) {
    int64_t n = 0;
    for (int64_t i = 0; i < src->len; i++) {
        n += keep[i] != 0;
    }
    tl_vec *out = tl_vec_new(n, (int64_t)sizeof(int64_t));
    int64_t j = 0;
    for (int64_t i = 0; i < src->len; i++) {
        if (keep[i]) {
            tl_vec_set(out, j++, tl_vec_get(src, i));
        }
    }
    return out;
}

int8_t *tl_mask_new(int64_t len) {
    return len > 0 ? tl_alloc((size_t)len) : NULL;
}

void tl_mask_set(int8_t *mask, int64_t i, int64_t value) {
    mask[i] = value != 0;
}

/* JSON string escaping, matching what the Lua and JavaScript printers emit. Anything below
 * space goes out as \u00xx, which is what both of the others do for control characters. */
tl_str *tl_quote(const tl_str *s) {
    /* Worst case every byte becomes \u00xx, plus the two quotes. */
    char *out = tl_alloc((size_t)(s->len * 6 + 2));
    int64_t n = 0;
    out[n++] = '"';
    for (int64_t i = 0; i < s->len; i++) {
        unsigned char c = (unsigned char)s->ptr[i];
        switch (c) {
            case '"': out[n++] = '\\'; out[n++] = '"'; break;
            case '\\': out[n++] = '\\'; out[n++] = '\\'; break;
            case '\n': out[n++] = '\\'; out[n++] = 'n'; break;
            case '\r': out[n++] = '\\'; out[n++] = 'r'; break;
            case '\t': out[n++] = '\\'; out[n++] = 't'; break;
            default:
                if (c < 0x20) {
                    n += snprintf(out + n, 7, "\\u%04x", c);
                } else {
                    out[n++] = (char)c;
                }
        }
    }
    out[n++] = '"';
    return tl_str_new(out, n);
}

/* Concatenate `parts`, which holds tl_str pointers, between `open` and `close` with `sep`
 * between elements. One allocation, so printing a Vec is not quadratic in its length. */
tl_str *tl_str_join(const tl_vec *parts, const tl_str *open, const tl_str *sep,
                    const tl_str *close) {
    int64_t total = open->len + close->len;
    for (int64_t i = 0; i < parts->len; i++) {
        total += ((const tl_str *)tl_vec_get(parts, i))->len;
        if (i > 0) {
            total += sep->len;
        }
    }

    char *out = tl_alloc((size_t)total);
    int64_t n = 0;
    memcpy(out + n, open->ptr, (size_t)open->len);
    n += open->len;
    for (int64_t i = 0; i < parts->len; i++) {
        if (i > 0) {
            memcpy(out + n, sep->ptr, (size_t)sep->len);
            n += sep->len;
        }
        const tl_str *p = (const tl_str *)tl_vec_get(parts, i);
        memcpy(out + n, p->ptr, (size_t)p->len);
        n += p->len;
    }
    memcpy(out + n, close->ptr, (size_t)close->len);
    n += close->len;
    return tl_str_new(out, n);
}
