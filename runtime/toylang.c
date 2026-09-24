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

/* pipe2, for a close-on-exec pipe with no window between creating it and marking it. */
#define _GNU_SOURCE

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <spawn.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

/* A toylang Str: bytes and a length, never null-terminated by contract. The bytes a literal
 * points at happen to carry a trailing NUL so that a debugger can print them, but len excludes
 * it and no code here reads past len. */
typedef struct {
    const char *ptr;
    int64_t len;
} tl_str;

/* runtime-rs's TlStr is defined to this layout and checks the same offsets. */
_Static_assert(sizeof(tl_str) == 16 && offsetof(tl_str, len) == 8, "tl_str layout");

/* Defined in runtime-rs, which owns the Str primitives. tl_str_new takes ownership of `bytes`
 * (a tl_alloc block) and copies nothing. */
tl_str *tl_str_new(char *bytes, int64_t len);

static void *tl_alloc(size_t n) {
    void *p = malloc(n);
    if (p == NULL) {
        const char *msg = "toylang: out of memory\n";
        write(2, msg, strlen(msg));
        exit(1);
    }
    return p;
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

/* Defined in runtime-rs, which owns the Vec, mask, record and Opt core: construction, the
 * element accessors, columns, masks. C reads a tl_vec's fields directly through the typedef
 * above, so runtime-rs's TlVec is defined to this layout and checks the same offsets. */
_Static_assert(sizeof(tl_vec) == 24 && offsetof(tl_vec, len) == 0 &&
                   offsetof(tl_vec, ncols) == 8 && offsetof(tl_vec, cols) == 16,
               "tl_vec layout");

tl_vec *tl_vec_new(int64_t len, int64_t ncols);
int64_t *tl_rec_new(int64_t nfields);

/* Defined in runtime-rs: `toylang: input: <what> at <path>` on stderr, then exit 1. */
_Noreturn void tl_fail(const char *what, const char *path);
int tl_utf8_valid(const char *s, size_t len);

/* A growable list of slots, used while a Vec's length is still unknown. */
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

/* pipe_through. One poll loop feeds the child's stdin and drains its stdout and stderr, so no
 * pipe can fill up and stall either side, and a child that never reads stdin cannot hang the
 * program: the write end is nonblocking and is closed once the child hangs up or everything
 * has been sent. */
extern char **environ;

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} tl_buf;

static void tl_buf_append(tl_buf *b, const char *src, size_t n) {
    if (b->len + n > b->cap) {
        b->cap = (b->len + n) * 2 + 64;
        b->data = realloc(b->data, b->cap);
        if (b->data == NULL) {
            fputs("toylang: out of memory\n", stderr);
            exit(1);
        }
    }
    memcpy(b->data + b->len, src, n);
    b->len += n;
}

/* Splits on '\n' only, keeping a '\r', and pushes each line as a PipeLine box: slot 0 the
 * variant tag, slot 1 a one-field record holding the text, the same layout the emitter builds
 * for `Stdout{text}`. A final line with no newline is still a line; empty output has none. */
static void tl_pipe_lines(tl_list *out, const tl_buf *b, int64_t tag) {
    size_t start = 0;
    while (start < b->len) {
        const char *nl = memchr(b->data + start, '\n', b->len - start);
        size_t end = nl != NULL ? (size_t)(nl - b->data) : b->len;
        if (!tl_utf8_valid(b->data + start, end - start)) {
            tl_fail("subprocess output is not valid UTF-8", "pipe_through");
        }
        char *bytes = tl_alloc(end - start);
        memcpy(bytes, b->data + start, end - start);
        int64_t *payload = tl_rec_new(1);
        payload[0] = (int64_t)tl_str_new(bytes, (int64_t)(end - start));
        int64_t *box = tl_rec_new(2);
        box[0] = tag;
        box[1] = (int64_t)payload;
        tl_list_push(out, (int64_t)box);
        start = end + 1;
    }
}

static void tl_pipe_close(int *fd) {
    if (*fd >= 0) {
        close(*fd);
        *fd = -1;
    }
}

tl_vec *tl_pipe_through(const tl_str *cmd, const tl_vec *args, const tl_vec *lines,
                        int64_t stdout_tag, int64_t stderr_tag) {
    tl_buf in = {NULL, 0, 0};
    for (int64_t i = 0; i < lines->len; i++) {
        const tl_str *line = (const tl_str *)lines->cols[0][i];
        tl_buf_append(&in, line->ptr, (size_t)line->len);
        tl_buf_append(&in, "\n", 1);
    }

    char **argv = tl_alloc((size_t)(args->len + 2) * sizeof(char *));
    for (int64_t i = 0; i < args->len + 1; i++) {
        const tl_str *s = i == 0 ? cmd : (const tl_str *)args->cols[0][i - 1];
        argv[i] = tl_alloc((size_t)s->len + 1);
        memcpy(argv[i], s->ptr, (size_t)s->len);
        argv[i][s->len] = '\0';
    }
    argv[args->len + 1] = NULL;

    int to_child[2], from_out[2], from_err[2];
    if (pipe2(to_child, O_CLOEXEC) != 0 || pipe2(from_out, O_CLOEXEC) != 0 ||
        pipe2(from_err, O_CLOEXEC) != 0) {
        tl_fail("cannot create pipes", "pipe_through");
    }

    /* Ignored signals survive exec, so the child gets SIGPIPE's default back: `yes | head`
     * style children rely on dying of it. */
    posix_spawn_file_actions_t fa;
    posix_spawnattr_t attr;
    sigset_t defaults;
    sigemptyset(&defaults);
    sigaddset(&defaults, SIGPIPE);
    posix_spawn_file_actions_init(&fa);
    posix_spawn_file_actions_adddup2(&fa, to_child[0], 0);
    posix_spawn_file_actions_adddup2(&fa, from_out[1], 1);
    posix_spawn_file_actions_adddup2(&fa, from_err[1], 2);
    posix_spawnattr_init(&attr);
    posix_spawnattr_setsigdefault(&attr, &defaults);
    posix_spawnattr_setflags(&attr, POSIX_SPAWN_SETSIGDEF);

    pid_t pid;
    int rc = posix_spawnp(&pid, argv[0], &fa, &attr, argv, environ);
    if (rc != 0) {
        fprintf(stderr, "toylang: cannot spawn subprocess `%s`: %s\n", argv[0], strerror(rc));
        exit(1);
    }
    posix_spawn_file_actions_destroy(&fa);
    posix_spawnattr_destroy(&attr);
    close(to_child[0]);
    close(from_out[1]);
    close(from_err[1]);

    /* A write to a child that already exited must fail with EPIPE, not kill this process. */
    void (*old_sigpipe)(int) = signal(SIGPIPE, SIG_IGN);
    int wfd = to_child[1], ofd = from_out[0], efd = from_err[0];
    fcntl(wfd, F_SETFL, fcntl(wfd, F_GETFL) | O_NONBLOCK);
    size_t sent = 0;
    tl_buf out = {NULL, 0, 0}, err = {NULL, 0, 0};
    if (in.len == 0) {
        tl_pipe_close(&wfd);
    }
    while (wfd >= 0 || ofd >= 0 || efd >= 0) {
        struct pollfd fds[3] = {{wfd, POLLOUT, 0}, {ofd, POLLIN, 0}, {efd, POLLIN, 0}};
        if (poll(fds, 3, -1) < 0) {
            if (errno == EINTR) {
                continue;
            }
            tl_fail("poll failed", "pipe_through");
        }
        if (wfd >= 0 && fds[0].revents != 0) {
            ssize_t n = write(wfd, in.data + sent, in.len - sent);
            if (n > 0) {
                sent += (size_t)n;
            } else if (n < 0 && errno != EAGAIN && errno != EINTR) {
                /* EPIPE: the child closed stdin early (head, sort -u), which is normal. */
                sent = in.len;
            }
            if (sent == in.len) {
                tl_pipe_close(&wfd);
            }
        }
        for (int k = 1; k <= 2; k++) {
            int *fd = k == 1 ? &ofd : &efd;
            tl_buf *sink = k == 1 ? &out : &err;
            if (*fd < 0 || fds[k].revents == 0) {
                continue;
            }
            char chunk[65536];
            ssize_t n = read(*fd, chunk, sizeof chunk);
            if (n > 0) {
                tl_buf_append(sink, chunk, (size_t)n);
            } else if (n == 0 || (errno != EINTR && errno != EAGAIN)) {
                tl_pipe_close(fd);
            }
        }
    }
    signal(SIGPIPE, old_sigpipe);
    while (waitpid(pid, NULL, 0) < 0 && errno == EINTR) {
    }

    tl_list items = {NULL, 0, 0};
    tl_pipe_lines(&items, &out, stdout_tag);
    tl_pipe_lines(&items, &err, stderr_tag);
    tl_vec *v = tl_vec_new(items.len, 1);
    for (int64_t i = 0; i < items.len; i++) {
        v->cols[0][i] = items.data[i];
    }
    free(in.data);
    free(out.data);
    free(err.data);
    return v;
}
