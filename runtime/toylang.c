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

/* A Vec: a length and `ncols` columns, each holding `len` raw 8-byte slots.
 *
 * The layout is struct of arrays. A Vec of scalars has one column; a Vec of records has one
 * column per field, which is what makes reading a field off a Vec a column rather than a gather.
 *
 * Every scalar toylang has fits one slot: an Int is an i64, and a Str, a record or a nested Vec
 * is a pointer. That is what lets one set of functions serve every element type instead of one
 * per width.
 */
typedef struct {
    int64_t len;
    int64_t ncols;
    int64_t **cols;
} tl_vec;

tl_vec *tl_vec_new(int64_t len, int64_t ncols) {
    tl_vec *v = tl_alloc(sizeof(tl_vec));
    v->len = len;
    v->ncols = ncols;
    v->cols = tl_alloc((size_t)ncols * sizeof(int64_t *));
    for (int64_t c = 0; c < ncols; c++) {
        v->cols[c] = len > 0 ? tl_alloc((size_t)len * sizeof(int64_t)) : NULL;
    }
    return v;
}

int64_t tl_vec_len(const tl_vec *v) {
    return v->len;
}

int64_t tl_vec_get(const tl_vec *v, int64_t col, int64_t i) {
    return v->cols[col][i];
}

void tl_vec_set(tl_vec *v, int64_t col, int64_t i, int64_t value) {
    v->cols[col][i] = value;
}

/* One field of a Vec of records, as a Vec of that field's type.
 *
 * The column is shared rather than copied, so `.name` on a Vec<User> costs one small header and
 * no element work. This is the whole reason the layout is struct of arrays.
 */
tl_vec *tl_vec_column(const tl_vec *v, int64_t col) {
    tl_vec *out = tl_alloc(sizeof(tl_vec));
    out->len = v->len;
    out->ncols = 1;
    out->cols = tl_alloc(sizeof(int64_t *));
    out->cols[0] = v->cols[col];
    return out;
}

/* select in two passes: count survivors, then fill. Two passes over a mask beats growing an
 * array, and every column is compacted with the same surviving indices. */
tl_vec *tl_vec_from_mask(const tl_vec *src, const int8_t *keep) {
    int64_t n = 0;
    for (int64_t i = 0; i < src->len; i++) {
        n += keep[i] != 0;
    }
    tl_vec *out = tl_vec_new(n, src->ncols);
    int64_t j = 0;
    for (int64_t i = 0; i < src->len; i++) {
        if (keep[i]) {
            for (int64_t c = 0; c < src->ncols; c++) {
                out->cols[c][j] = src->cols[c][i];
            }
            j++;
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

/* A record: one slot per field, in the field order the type declares. Records only ever
 * arrive from input, since the language has no expression that builds one. */
int64_t *tl_rec_new(int64_t nfields) {
    return tl_alloc((size_t)nfields * sizeof(int64_t));
}

int64_t tl_rec_get(const int64_t *r, int64_t field) {
    return r[field];
}

void tl_rec_set(int64_t *r, int64_t field, int64_t value) {
    r[field] = value;
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
        total += ((const tl_str *)tl_vec_get(parts, 0, i))->len;
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
        const tl_str *p = (const tl_str *)tl_vec_get(parts, 0, i);
        memcpy(out + n, p->ptr, (size_t)p->len);
        n += p->len;
    }
    memcpy(out + n, close->ptr, (size_t)close->len);
    n += close->len;
    return tl_str_new(out, n);
}

/* Reading input.
 *
 * The parser is driven by a type descriptor the compiler emits as one string, so it only ever
 * looks for the shape the program declared. The grammar is:
 *
 *   s            Str
 *   i            Int
 *   b            Bool
 *   [T           Vec of T
 *   {n,name:T,...}   record with n fields, in the type's declared order
 *   e{n,Name,variant,variant:T,...}   enum with n variants, in declaration order (the
 *                variant's position is its tag); a variant with no `:T` is a unit variant.
 *                Name is what a mismatch says it expected, and what `@` names.
 *   @Name        the enum called Name, whose descriptor is still open around this one. A
 *                recursive enum's payload names itself back rather than spelling itself out
 *                again, which would have no end (kantord/toylang#94).
 *
 * It re-validates rather than trusting a pre-checked shape, so a built binary works on its own
 * with `./adults < data.json` and not only under the test harness. A mismatch names the path
 * that failed and exits non-zero, which is what the Rust-side check does too.
 */

typedef struct {
    const char *p;
    const char *end;
} tl_json;

/* The enums whose descriptors are open around the type being parsed, innermost first. `@Name`
 * resolves by walking up this chain, so a name is always the nearest enclosing enum of that
 * name -- which is what makes two instantiations of one generic enum, nested inside each other,
 * resolve to the right one. */
typedef struct tl_open_enum {
    const char *name;
    int64_t name_len;
    /* The `e{...}` this name was opened at, which is what `@Name` parses as. */
    const char *desc;
    const struct tl_open_enum *up;
} tl_open_enum;

static void tl_fail(const char *what, const char *path) {
    char buf[512];
    int n = snprintf(buf, sizeof buf, "toylang: input: %s at %s\n", what,
                     path[0] ? path : "input");
    (void)!write(2, buf, (size_t)n);
    exit(1);
}

static void tl_skip_ws(tl_json *j) {
    while (j->p < j->end && (*j->p == ' ' || *j->p == '\t' || *j->p == '\n' || *j->p == '\r')) {
        j->p++;
    }
}

static void tl_expect(tl_json *j, char c, const char *path) {
    tl_skip_ws(j);
    if (j->p >= j->end || *j->p != c) {
        char what[32];
        snprintf(what, sizeof what, "expected `%c`", c);
        tl_fail(what, path);
    }
    j->p++;
}

/* A growable list of slots, used while an array's length is still unknown. */
typedef struct {
    int64_t *data;
    int64_t len;
    int64_t cap;
} tl_list;

static void tl_list_push(tl_list *l, int64_t v) {
    if (l->len == l->cap) {
        int64_t cap = l->cap ? l->cap * 2 : 8;
        int64_t *data = tl_alloc((size_t)cap * sizeof(int64_t));
        memcpy(data, l->data, (size_t)l->len * sizeof(int64_t));
        l->data = data;
        l->cap = cap;
    }
    l->data[l->len++] = v;
}

/* Advance `t` past one complete type in the descriptor. */
static const char *tl_type_skip(const char *t) {
    switch (*t) {
        case 's': case 'i': case 'b':
            return t + 1;
        case '[':
            return tl_type_skip(t + 1);
        case '@':
            /* A back-reference is one token: the name runs to whatever delimits the type. */
            t++;
            while (*t != ',' && *t != '}') {
                t++;
            }
            return t;
        case '{': {
            t++;
            int64_t n = 0;
            while (*t != ',') {
                n = n * 10 + (*t++ - '0');
            }
            t++;
            for (int64_t f = 0; f < n; f++) {
                while (*t != ':') {
                    t++;
                }
                t = tl_type_skip(t + 1);
                if (*t == ',') {
                    t++;
                }
            }
            return t + 1; /* past '}' */
        }
        case 'e': {
            t += 2; /* past "e{" */
            int64_t n = 0;
            while (*t != ',') {
                n = n * 10 + (*t++ - '0');
            }
            t++;
            while (*t != ',' && *t != '}') {
                t++; /* the enum's name */
            }
            for (int64_t v = 0; v < n; v++) {
                t++; /* past ',' */
                while (*t != ':' && *t != ',' && *t != '}') {
                    t++;
                }
                if (*t == ':') {
                    t = tl_type_skip(t + 1);
                }
            }
            return t + 1; /* past '}' */
        }
        default:
            tl_fail("bad type descriptor", "");
            return t;
    }
}

static int64_t tl_parse(tl_json *j, const char *t, const char *path,
                        const tl_open_enum *scope);

/* Consumes exactly 4 hex digits at j->p and returns them as a UTF-16 code unit. */
static uint32_t tl_hex4(tl_json *j, const char *path) {
    uint32_t v = 0;
    for (int i = 0; i < 4; i++) {
        if (j->p >= j->end) {
            tl_fail("unterminated \\u escape", path);
        }
        char c = *j->p;
        int d = (c >= '0' && c <= '9')   ? c - '0'
                : (c >= 'a' && c <= 'f') ? c - 'a' + 10
                : (c >= 'A' && c <= 'F') ? c - 'A' + 10
                                         : -1;
        if (d < 0) {
            tl_fail("bad \\u escape", path);
        }
        v = v * 16 + (uint32_t)d;
        j->p++;
    }
    return v;
}

/* Appends cp's UTF-8 encoding to out, which tl_parse_string has already sized generously
 * enough: every multi-byte encoding here is shorter than the \u escape(s) that produced it. */
static void tl_utf8_encode(char *out, int64_t *n, uint32_t cp) {
    if (cp < 0x80) {
        out[(*n)++] = (char)cp;
    } else if (cp < 0x800) {
        out[(*n)++] = (char)(0xC0 | (cp >> 6));
        out[(*n)++] = (char)(0x80 | (cp & 0x3F));
    } else if (cp < 0x10000) {
        out[(*n)++] = (char)(0xE0 | (cp >> 12));
        out[(*n)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[(*n)++] = (char)(0x80 | (cp & 0x3F));
    } else {
        out[(*n)++] = (char)(0xF0 | (cp >> 18));
        out[(*n)++] = (char)(0x80 | ((cp >> 12) & 0x3F));
        out[(*n)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[(*n)++] = (char)(0x80 | (cp & 0x3F));
    }
}

static tl_str *tl_parse_string(tl_json *j, const char *path) {
    tl_expect(j, '"', path);
    char *out = tl_alloc((size_t)(j->end - j->p));
    int64_t n = 0;
    while (j->p < j->end && *j->p != '"') {
        if (*j->p == '\\') {
            j->p++;
            if (j->p >= j->end) {
                tl_fail("unterminated escape", path);
            }
            // A control character with no named shorthand (NUL, say) is the only escape
            // that reaches this in practice: src/lib.rs re-serializes input through
            // serde_json before any backend sees it, and that writer emits raw UTF-8 for
            // everything else. A surrogate pair is still combined into one codepoint below,
            // to be a JSON decoder rather than one tied to that upstream behavior.
            if (*j->p == 'u') {
                j->p++;
                uint32_t cu = tl_hex4(j, path);
                uint32_t cp;
                if (cu >= 0xD800 && cu <= 0xDBFF) {
                    if (j->p + 1 >= j->end || j->p[0] != '\\' || j->p[1] != 'u') {
                        tl_fail("unpaired surrogate", path);
                    }
                    j->p += 2;
                    uint32_t lo = tl_hex4(j, path);
                    if (lo < 0xDC00 || lo > 0xDFFF) {
                        tl_fail("unpaired surrogate", path);
                    }
                    cp = 0x10000 + ((cu - 0xD800) << 10) + (lo - 0xDC00);
                } else if (cu >= 0xDC00 && cu <= 0xDFFF) {
                    tl_fail("unpaired surrogate", path);
                } else {
                    cp = cu;
                }
                tl_utf8_encode(out, &n, cp);
                continue;
            }
            switch (*j->p) {
                case '"': out[n++] = '"'; break;
                case '\\': out[n++] = '\\'; break;
                case '/': out[n++] = '/'; break;
                case 'n': out[n++] = '\n'; break;
                case 't': out[n++] = '\t'; break;
                case 'r': out[n++] = '\r'; break;
                case 'b': out[n++] = '\b'; break;
                case 'f': out[n++] = '\f'; break;
                default: tl_fail("unsupported escape", path);
            }
            j->p++;
        } else {
            out[n++] = *j->p++;
        }
    }
    tl_expect(j, '"', path);
    return tl_str_new(out, n);
}

/* Skip one JSON value without interpreting it, for fields the type does not declare. */
static void tl_skip_value(tl_json *j, const char *path) {
    tl_skip_ws(j);
    if (j->p >= j->end) {
        tl_fail("unexpected end of input", path);
    }
    if (*j->p == '"') {
        tl_parse_string(j, path);
        return;
    }
    if (*j->p == '[' || *j->p == '{') {
        char open = *j->p;
        char close = open == '[' ? ']' : '}';
        int depth = 0;
        while (j->p < j->end) {
            if (*j->p == '"') {
                tl_parse_string(j, path);
                continue;
            }
            if (*j->p == open) {
                depth++;
            } else if (*j->p == close) {
                depth--;
                if (depth == 0) {
                    j->p++;
                    return;
                }
            }
            j->p++;
        }
        tl_fail("unterminated value", path);
    }
    while (j->p < j->end && *j->p != ',' && *j->p != '}' && *j->p != ']') {
        j->p++;
    }
}

static int64_t tl_parse_record(tl_json *j, const char *t, const char *path,
                               const tl_open_enum *scope) {
    /* Field names and types, read off the descriptor once. */
    t++;
    int64_t n = 0;
    while (*t != ',') {
        n = n * 10 + (*t++ - '0');
    }
    t++;

    const char *names[64];
    int64_t name_lens[64];
    const char *types[64];
    if (n > 64) {
        tl_fail("too many record fields", path);
    }
    for (int64_t f = 0; f < n; f++) {
        names[f] = t;
        while (*t != ':') {
            t++;
        }
        name_lens[f] = t - names[f];
        types[f] = t + 1;
        t = tl_type_skip(t + 1);
        if (*t == ',') {
            t++;
        }
    }

    int64_t *rec = tl_rec_new(n);
    int8_t seen[64];
    memset(seen, 0, sizeof seen);

    tl_expect(j, '{', path);
    tl_skip_ws(j);
    if (j->p < j->end && *j->p == '}') {
        j->p++;
    } else {
        for (;;) {
            tl_str *key = tl_parse_string(j, path);
            tl_expect(j, ':', path);

            int64_t match = -1;
            for (int64_t f = 0; f < n; f++) {
                if (key->len == name_lens[f] && memcmp(key->ptr, names[f], (size_t)key->len) == 0) {
                    match = f;
                    break;
                }
            }
            if (match < 0) {
                /* Fields the program did not declare are ignored, matching the Rust-side check. */
                tl_skip_value(j, path);
            } else {
                char sub[256];
                snprintf(sub, sizeof sub, "%s.%.*s", path, (int)key->len, key->ptr);
                rec[match] = tl_parse(j, types[match], sub, scope);
                seen[match] = 1;
            }

            tl_skip_ws(j);
            if (j->p < j->end && *j->p == ',') {
                j->p++;
                continue;
            }
            break;
        }
        tl_expect(j, '}', path);
    }

    for (int64_t f = 0; f < n; f++) {
        if (!seen[f]) {
            char what[256];
            snprintf(what, sizeof what, "missing field `%.*s`", (int)name_lens[f], names[f]);
            tl_fail(what, path);
        }
    }
    return (int64_t)rec;
}

/* One enum value spans two JSON shapes (ADR 0009): a bare string for a unit variant, a
 * single-key object for a payload one. What comes back is the same two-slot box the compiler
 * builds for a constructed enum: slot 0 the variant's declaration index, slot 1 the payload. */
static int64_t tl_parse_enum(tl_json *j, const char *t, const char *path,
                             const tl_open_enum *scope) {
    const char *self = t;
    t += 2; /* past "e{" */
    int64_t n = 0;
    while (*t != ',') {
        n = n * 10 + (*t++ - '0');
    }
    t++;
    const char *ename = t;
    while (*t != ',' && *t != '}') {
        t++;
    }
    int64_t ename_len = t - ename;

    const char *names[64];
    int64_t name_lens[64];
    const char *types[64]; /* NULL marks a unit variant */
    if (n > 64) {
        tl_fail("too many variants", path);
    }
    for (int64_t v = 0; v < n; v++) {
        t++;
        names[v] = t;
        while (*t != ':' && *t != ',' && *t != '}') {
            t++;
        }
        name_lens[v] = t - names[v];
        if (*t == ':') {
            types[v] = t + 1;
            t = tl_type_skip(t + 1);
        } else {
            types[v] = NULL;
        }
    }

    tl_skip_ws(j);
    if (j->p < j->end && *j->p == '"') {
        tl_str *s = tl_parse_string(j, path);
        for (int64_t v = 0; v < n; v++) {
            if (types[v] == NULL && s->len == name_lens[v] &&
                memcmp(s->ptr, names[v], (size_t)s->len) == 0) {
                int64_t *box = tl_rec_new(2);
                box[0] = v;
                return (int64_t)box;
            }
        }
        char what[256];
        snprintf(what, sizeof what, "`%.*s` is not a unit variant of %.*s", (int)s->len,
                 s->ptr, (int)ename_len, ename);
        tl_fail(what, path);
    }
    if (j->p < j->end && *j->p == '{') {
        j->p++;
        tl_str *key = tl_parse_string(j, path);
        tl_expect(j, ':', path);
        for (int64_t v = 0; v < n; v++) {
            if (types[v] != NULL && key->len == name_lens[v] &&
                memcmp(key->ptr, names[v], (size_t)key->len) == 0) {
                char sub[256];
                snprintf(sub, sizeof sub, "%s.%.*s", path, (int)key->len, key->ptr);
                int64_t *box = tl_rec_new(2);
                box[0] = v;
                /* Open around the payload, so a `@` inside it names this enum. */
                tl_open_enum open = {ename, ename_len, self, scope};
                box[1] = tl_parse(j, types[v], sub, &open);
                /* One key is the whole shape, so the wrapper closes right here. */
                tl_expect(j, '}', path);
                return (int64_t)box;
            }
        }
        char what[256];
        snprintf(what, sizeof what, "`%.*s` is not a payload variant of %.*s", (int)key->len,
                 key->ptr, (int)ename_len, ename);
        tl_fail(what, path);
    }
    char what[256];
    snprintf(what, sizeof what, "expected %.*s", (int)ename_len, ename);
    tl_fail(what, path);
    return 0;
}

/* A Vec of records is parsed element by element and then transposed into columns. Filling the
 * columns directly would avoid materialising each record, and is worth doing when the parser
 * stops being the cheapest part of reading input. */
static int64_t tl_parse_vec(tl_json *j, const char *t, const char *path,
                            const tl_open_enum *scope) {
    const char *elem = t + 1;
    /* Whether the element is a record is a separate question from how many columns it has: a
     * record with one field also has one column, so the count cannot stand in for the test. */
    int is_record = *elem == '{';
    int64_t ncols = 1;
    if (is_record) {
        const char *c = elem + 1;
        ncols = 0;
        while (*c != ',') {
            ncols = ncols * 10 + (*c++ - '0');
        }
    }

    tl_list items = {NULL, 0, 0};
    tl_expect(j, '[', path);
    tl_skip_ws(j);
    if (j->p < j->end && *j->p == ']') {
        j->p++;
    } else {
        for (;;) {
            char sub[256];
            snprintf(sub, sizeof sub, "%s[%lld]", path, (long long)items.len);
            tl_list_push(&items, tl_parse(j, elem, sub, scope));
            tl_skip_ws(j);
            if (j->p < j->end && *j->p == ',') {
                j->p++;
                continue;
            }
            break;
        }
        tl_expect(j, ']', path);
    }

    tl_vec *v = tl_vec_new(items.len, ncols);
    for (int64_t i = 0; i < items.len; i++) {
        if (!is_record) {
            v->cols[0][i] = items.data[i];
        } else {
            const int64_t *rec = (const int64_t *)items.data[i];
            for (int64_t c = 0; c < ncols; c++) {
                v->cols[c][i] = rec[c];
            }
        }
    }
    return (int64_t)v;
}

static int64_t tl_parse(tl_json *j, const char *t, const char *path,
                        const tl_open_enum *scope) {
    tl_skip_ws(j);
    if (j->p >= j->end) {
        tl_fail("unexpected end of input", path);
    }
    switch (*t) {
        case 's': {
            if (*j->p != '"') {
                tl_fail("expected a string", path);
            }
            return (int64_t)tl_parse_string(j, path);
        }
        case 'i': {
            const char *start = j->p;
            if (j->p < j->end && (*j->p == '-' || *j->p == '+')) {
                j->p++;
            }
            while (j->p < j->end && *j->p >= '0' && *j->p <= '9') {
                j->p++;
            }
            if (j->p == start) {
                tl_fail("expected an integer", path);
            }
            /* A float where Int was declared is an error, not a truncation. */
            if (j->p < j->end && (*j->p == '.' || *j->p == 'e' || *j->p == 'E')) {
                tl_fail("expected an integer, found a non-integer number", path);
            }
            char buf[32];
            int64_t n = j->p - start;
            if (n >= (int64_t)sizeof buf) {
                tl_fail("integer is out of range", path);
            }
            memcpy(buf, start, (size_t)n);
            buf[n] = 0;
            return strtoll(buf, NULL, 10);
        }
        case 'b': {
            if (j->end - j->p >= 4 && memcmp(j->p, "true", 4) == 0) {
                j->p += 4;
                return 1;
            }
            if (j->end - j->p >= 5 && memcmp(j->p, "false", 5) == 0) {
                j->p += 5;
                return 0;
            }
            tl_fail("expected a boolean", path);
            return 0;
        }
        case '[':
            if (*j->p != '[') {
                tl_fail("expected an array", path);
            }
            return tl_parse_vec(j, t, path, scope);
        case '{':
            if (*j->p != '{') {
                tl_fail("expected an object", path);
            }
            return tl_parse_record(j, t, path, scope);
        case 'e':
            /* Which of its two shapes arrived is tl_parse_enum's own question. */
            return tl_parse_enum(j, t, path, scope);
        case '@': {
            /* The enum this names is open around here, and its descriptor is where to resume. */
            const char *name = t + 1;
            int64_t len = 0;
            while (name[len] != ',' && name[len] != '}') {
                len++;
            }
            for (const tl_open_enum *e = scope; e != NULL; e = e->up) {
                if (e->name_len == len && memcmp(e->name, name, (size_t)len) == 0) {
                    return tl_parse(j, e->desc, path, scope);
                }
            }
            tl_fail("bad type descriptor", path);
            return 0;
        }
        default:
            tl_fail("bad type descriptor", path);
            return 0;
    }
}

int64_t tl_read_input(const tl_str *descriptor) {
    size_t cap = 1 << 16;
    size_t len = 0;
    char *buf = tl_alloc(cap);
    for (;;) {
        if (len == cap) {
            cap *= 2;
            char *bigger = tl_alloc(cap);
            memcpy(bigger, buf, len);
            buf = bigger;
        }
        ssize_t n = read(0, buf + len, cap - len);
        if (n < 0) {
            tl_fail("could not read stdin", "");
        }
        if (n == 0) {
            break;
        }
        len += (size_t)n;
    }

    /* The descriptor is a literal the compiler emitted, so it is NUL-terminated in practice;
     * copy it anyway so the C string functions above have one for certain. */
    char *t = tl_alloc((size_t)descriptor->len + 1);
    memcpy(t, descriptor->ptr, (size_t)descriptor->len);
    t[descriptor->len] = 0;

    tl_json j = {buf, buf + len};
    int64_t value = tl_parse(&j, t, "input", NULL);
    tl_skip_ws(&j);
    if (j.p != j.end) {
        tl_fail("trailing content after the value", "input");
    }
    return value;
}

/* Every remaining JSON value on stdin, one per line, parsed with the same descriptor-driven
 * grammar tl_read_input uses above and assembled into a proper Vec -- spread into columns when
 * the element is a record, the same invariant vec_lit and tl_at already keep, since tl_parse
 * hands back a record as one packed blob rather than already-columnar data.
 *
 * A blank line is skipped rather than fed to tl_parse, which would fail trying to parse nothing.
 */
tl_vec *tl_read_inputs(const tl_str *descriptor) {
    char *t = tl_alloc((size_t)descriptor->len + 1);
    memcpy(t, descriptor->ptr, (size_t)descriptor->len);
    t[descriptor->len] = 0;

    int is_record = t[0] == '{';
    int64_t ncols = 1;
    if (is_record) {
        const char *p = t + 1;
        ncols = 0;
        while (*p != ',') {
            ncols = ncols * 10 + (*p++ - '0');
        }
    }

    tl_list items = {NULL, 0, 0};
    char *line = NULL;
    size_t cap = 0;
    ssize_t len;
    while ((len = getline(&line, &cap, stdin)) != -1) {
        tl_json j = {line, line + len};
        tl_skip_ws(&j);
        if (j.p == j.end) {
            continue;
        }
        j.p = line;
        int64_t value = tl_parse(&j, t, "inputs", NULL);
        tl_skip_ws(&j);
        if (j.p != j.end) {
            tl_fail("trailing content after the value", "inputs");
        }
        tl_list_push(&items, value);
    }
    free(line);

    tl_vec *out = tl_vec_new(items.len, ncols);
    for (int64_t i = 0; i < items.len; i++) {
        if (is_record) {
            const int64_t *rec = (const int64_t *)items.data[i];
            for (int64_t c = 0; c < ncols; c++) {
                out->cols[c][i] = rec[c];
            }
        } else {
            out->cols[0][i] = items.data[i];
        }
    }
    return out;
}

/* One JSON value from stdin per call, parsed the same descriptor-driven way tl_read_inputs
 * assembles its whole Vec from, but one line at a time and with no Vec ever built: the caller
 * drives its own loop and decides when to stop, which is what lets the native backend fuse
 * `jsonlines(f(inputs))` into a read-one/transform-one/write-one loop (see tir::fusion)
 * instead of collecting everything before printing anything.
 *
 * Returns 0 at EOF (*out untouched) and 1 with *out set otherwise. A separate flag rather than a
 * sentinel value in *out, because a record pointer or an Int result can legitimately be any
 * int64_t, including whatever a sentinel would need to reserve.
 *
 * getline's buffer is not reused across calls the way tl_read_inputs's is within its own single
 * call: this function is called once per record from the caller's own loop, so there is no one
 * long-lived buffer to reuse, and toylang optimises for none of this to begin with. */
int tl_read_one_input(const tl_str *descriptor, int64_t *out) {
    char *t = tl_alloc((size_t)descriptor->len + 1);
    memcpy(t, descriptor->ptr, (size_t)descriptor->len);
    t[descriptor->len] = 0;

    char *line = NULL;
    size_t cap = 0;
    for (;;) {
        ssize_t len = getline(&line, &cap, stdin);
        if (len == -1) {
            free(line);
            return 0;
        }
        tl_json j = {line, line + len};
        tl_skip_ws(&j);
        if (j.p == j.end) {
            continue;
        }
        j.p = line;
        int64_t value = tl_parse(&j, t, "inputs", NULL);
        tl_skip_ws(&j);
        if (j.p != j.end) {
            tl_fail("trailing content after the value", "inputs");
        }
        *out = value;
        free(line);
        return 1;
    }
}

/* One raw line of stdin at a time: the streaming counterpart of tl_collect_lines below, the
 * same way tl_read_one_input above is tl_read_inputs's. Returns 0 at EOF (*out untouched) and
 * 1 with *out set to the line's tl_str otherwise. Same trailing-newline rule as
 * tl_collect_lines; a blank line is a line, since `lines` keeps them. */
int tl_read_one_line(int64_t *out) {
    char *line = NULL;
    size_t cap = 0;
    ssize_t len = getline(&line, &cap, stdin);
    if (len == -1) {
        free(line);
        return 0;
    }
    if (len > 0 && line[len - 1] == '\n') {
        len--;
    }
    char *bytes = tl_alloc((size_t)len);
    memcpy(bytes, line, (size_t)len);
    *out = (int64_t)tl_str_new(bytes, len);
    free(line);
    return 1;
}

/* One line of stdin at a time via getline, in contrast to tl_read_input just above, which reads
 * the whole stream before anything else can run. getline reuses the same growable buffer across
 * calls, so each line is copied out to its own allocation before being stored; the buffer would
 * otherwise be overwritten, and every previously stored line, on the next call.
 *
 * The trailing newline getline includes is stripped; a final line with none is still yielded,
 * matching wc -l's undercount being the mistake to avoid rather than the convention to follow.
 * A bare \r is left untouched as ordinary content, matching jq -R and Python's own stdin
 * iteration, neither of which treats CRLF specially. */
tl_vec *tl_collect_lines(void) {
    tl_list items = {NULL, 0, 0};
    char *line = NULL;
    size_t cap = 0;
    ssize_t len;
    while ((len = getline(&line, &cap, stdin)) != -1) {
        if (len > 0 && line[len - 1] == '\n') {
            len--;
        }
        char *bytes = tl_alloc((size_t)len);
        memcpy(bytes, line, (size_t)len);
        tl_list_push(&items, (int64_t)tl_str_new(bytes, len));
    }
    free(line);

    tl_vec *v = tl_vec_new(items.len, 1);
    for (int64_t i = 0; i < items.len; i++) {
        v->cols[0][i] = items.data[i];
    }
    return v;
}

/* Gather one element of a Vec of records back into a record.
 *
 * The struct-of-arrays layout means an element is spread across columns, and almost nothing in
 * the language needs it whole: select reads single fields out of the columns, and `.field`
 * returns a column. Printing is the exception, because rendering an element needs every field
 * at once. This is the only gather, and it exists for output alone.
 */
int64_t *tl_rec_from_vec(const tl_vec *v, int64_t i) {
    int64_t *rec = tl_rec_new(v->ncols);
    for (int64_t c = 0; c < v->ncols; c++) {
        rec[c] = v->cols[c][i];
    }
    return rec;
}

/* Opt: a pointer to a slot, or NULL for absent.
 *
 * Boxing rather than a tag pair, because a slot holds any value an Int can and there is no
 * spare bit pattern to mean absent. Uniform across element types, which is what lets one
 * function serve them all.
 */
int64_t *tl_opt_some(int64_t value) {
    int64_t *p = tl_alloc(sizeof(int64_t));
    *p = value;
    return p;
}

int64_t tl_opt_is_some(const int64_t *o) {
    return o != NULL;
}

int64_t tl_opt_get(const int64_t *o) {
    return *o;
}

/* Every element but the first. NULL on an empty Vec -- the same absence encoding tl_opt_some
 * uses everywhere else, so a caller unwraps it the same way an Index result is unwrapped. */
int64_t *tl_vec_tail(const tl_vec *v) {
    if (v->len == 0) {
        return NULL;
    }
    tl_vec *out = tl_vec_new(v->len - 1, v->ncols);
    for (int64_t c = 0; c < v->ncols; c++) {
        if (out->len > 0) {
            memcpy(out->cols[c], v->cols[c] + 1, (size_t)out->len * sizeof(int64_t));
        }
    }
    return tl_opt_some((int64_t)out);
}

/* Flatten a Vec<Vec<T>> into a Vec<T>. `ncols` is T's column count: passed in rather than read
 * off an inner Vec, since an empty outer Vec has no inner Vec to read it from. */
tl_vec *tl_vec_concat(const tl_vec *vv, int64_t ncols) {
    int64_t total = 0;
    for (int64_t i = 0; i < vv->len; i++) {
        total += ((const tl_vec *)vv->cols[0][i])->len;
    }
    tl_vec *out = tl_vec_new(total, ncols);
    int64_t at = 0;
    for (int64_t i = 0; i < vv->len; i++) {
        const tl_vec *inner = (const tl_vec *)vv->cols[0][i];
        for (int64_t c = 0; c < ncols; c++) {
            if (inner->len > 0) {
                memcpy(out->cols[c] + at, inner->cols[c], (size_t)inner->len * sizeof(int64_t));
            }
        }
        at += inner->len;
    }
    return out;
}

/* Ascending by raw int64 value: what backs Int, Int64, and Char, since all three live in the
 * slot unnarrowed (a Char is a codepoint, and the checker already keeps it from mixing with the
 * others). qsort's comparator returns the sign of the difference rather than subtracting, since
 * an int64 difference can itself overflow the int qsort wants back. */
static int tl_cmp_int64(const void *a, const void *b) {
    int64_t x = *(const int64_t *)a;
    int64_t y = *(const int64_t *)b;
    return x < y ? -1 : (x > y ? 1 : 0);
}

static int tl_cmp_str_slot(const void *a, const void *b) {
    const tl_str *x = (const tl_str *)*(const int64_t *)a;
    const tl_str *y = (const tl_str *)*(const int64_t *)b;
    return (int)tl_str_cmp(x, y);
}

/* `sort` over a Vec whose element is Int, Int64, or Char: one column of raw int64 slots, sorted
 * in place on a copy. Restricted to these by the checker (`orderable`), so there is no column
 * beyond the one holding the elements themselves to carry along. */
tl_vec *tl_vec_sort_int(const tl_vec *v) {
    tl_vec *out = tl_vec_new(v->len, 1);
    if (v->len > 0) {
        memcpy(out->cols[0], v->cols[0], (size_t)v->len * sizeof(int64_t));
        qsort(out->cols[0], (size_t)v->len, sizeof(int64_t), tl_cmp_int64);
    }
    return out;
}

/* The same shape for `Vec<Str>`, whose slots are `tl_str *` rather than raw integers. */
tl_vec *tl_vec_sort_str(const tl_vec *v) {
    tl_vec *out = tl_vec_new(v->len, 1);
    if (v->len > 0) {
        memcpy(out->cols[0], v->cols[0], (size_t)v->len * sizeof(int64_t));
        qsort(out->cols[0], (size_t)v->len, sizeof(int64_t), tl_cmp_str_slot);
    }
    return out;
}

/* `reverse`, generic over the element type the way `tl_vec_tail` is: every column's row order
 * flips together, so a Vec of records or of nested Vecs reverses correctly with no type-specific
 * code. */
tl_vec *tl_vec_reverse(const tl_vec *v, int64_t ncols) {
    tl_vec *out = tl_vec_new(v->len, ncols);
    for (int64_t c = 0; c < ncols; c++) {
        for (int64_t i = 0; i < v->len; i++) {
            out->cols[c][i] = v->cols[c][v->len - 1 - i];
        }
    }
    return out;
}

/* Collapse one dimension at `i`, `depth` layers down, counting from the end when negative.
 *
 * `is_record` decides whether an entry has to be gathered out of the columns. The column count
 * cannot stand in for that test: a record with one field has one column exactly like a Vec of
 * scalars does.
 */
int64_t *tl_at(const tl_vec *v, int64_t i, int64_t depth, int is_record) {
    if (depth > 0) {
        tl_vec *out = tl_vec_new(v->len, 1);
        for (int64_t k = 0; k < v->len; k++) {
            const tl_vec *inner = (const tl_vec *)v->cols[0][k];
            out->cols[0][k] = (int64_t)tl_at(inner, i, depth - 1, is_record);
        }
        return (int64_t *)out;
    }
    if (i < 0) {
        i = v->len + i;
    }
    if (i < 0 || i >= v->len) {
        return NULL;
    }
    return tl_opt_some(is_record ? (int64_t)tl_rec_from_vec(v, i) : v->cols[0][i]);
}

/* Insist an Opt is present, `depth` layers down.
 *
 * Unlike an index this needs no is_record flag: an Opt already holds a gathered value, so there
 * is nothing left to collect out of columns.
 */
int64_t *tl_unwrap(int64_t *o, int64_t depth) {
    if (depth > 0) {
        tl_vec *v = (tl_vec *)o;
        tl_vec *out = tl_vec_new(v->len, 1);
        for (int64_t k = 0; k < v->len; k++) {
            out->cols[0][k] = (int64_t)tl_unwrap((int64_t *)v->cols[0][k], depth - 1);
        }
        return (int64_t *)out;
    }
    if (o == NULL) {
        const char *msg = "toylang: unwrapped a value that is not there\n";
        (void)!write(2, msg, strlen(msg));
        exit(1);
    }
    return (int64_t *)*o;
}

/* The only way arithmetic can fail. */
_Noreturn void tl_div_by_zero(void) {
    const char *msg = "toylang: divided by zero\n";
    (void)!write(2, msg, strlen(msg));
    exit(1);
}

/* Build a Vec of `len` scalars, filled by the caller. `map` produces one column whatever the
 * source had, since its result element is whatever the body returned. */
/* The integers from zero up to but not including n. A negative n gives an empty Vec rather
 * than an error, the same as asking for zero of them. */
tl_vec *tl_range(int64_t n) {
    if (n < 0) {
        n = 0;
    }
    tl_vec *v = tl_vec_new(n, 1);
    for (int64_t i = 0; i < n; i++) {
        v->cols[0][i] = i;
    }
    return v;
}

/* Every Unicode scalar value in s, decoded from UTF-8, one codepoint per element -- not one
 * byte and not one UTF-16 unit, so a character outside the Basic Multilingual Plane is one
 * element here even where a backend whose own strings are UTF-16 needs a surrogate pair to
 * spell it. `s->len` is an overallocation (the ASCII-only case needs exactly that many, and
 * every multi-byte codepoint needs fewer slots than the bytes it decodes from), trimmed to the
 * real count once decoding is done, the same sizing tl_parse_string already relies on for the
 * reverse direction. Malformed UTF-8 cannot occur: every tl_str this runtime builds is already
 * valid UTF-8, whether from a literal or from tl_utf8_encode. */
tl_vec *tl_chars(const tl_str *s) {
    tl_vec *v = tl_vec_new(s->len, 1);
    int64_t n = 0;
    int64_t i = 0;
    while (i < s->len) {
        unsigned char b0 = (unsigned char)s->ptr[i];
        uint32_t cp;
        int extra;
        if (b0 < 0x80) {
            cp = b0;
            extra = 0;
        } else if ((b0 & 0xE0) == 0xC0) {
            cp = b0 & 0x1F;
            extra = 1;
        } else if ((b0 & 0xF0) == 0xE0) {
            cp = b0 & 0x0F;
            extra = 2;
        } else {
            cp = b0 & 0x07;
            extra = 3;
        }
        i++;
        for (int k = 0; k < extra; k++) {
            cp = (cp << 6) | ((unsigned char)s->ptr[i] & 0x3F);
            i++;
        }
        v->cols[0][n++] = (int64_t)cp;
    }
    v->len = n;
    return v;
}
