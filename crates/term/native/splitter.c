/* Streaming terminal/media splitter ABI v1. A splitter pins its module until
 * freed. Input and callbacks are borrowed only during split; callbacks are
 * synchronous and serial. The grow/release callbacks use the caller's Rust
 * allocator. Media events transfer their allocation and capacity to the host,
 * even when the event callback returns failure; text event bytes are borrowed
 * only during the callback. A callback that returns zero poisons the state.
 * Returning 1 from split means plain-text state, 2 means a partial sequence,
 * and 0 indicates allocation/callback failure. Plain bytes without ESC in
 * text state are defined to pass through untouched and may bypass this ABI.
 * Published libraries are operator-trusted and run with process authority. */
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef int (*emit_fn)(void *, int, const uint8_t *, size_t, int, size_t);
typedef uint8_t *(*grow_fn)(uint8_t *, size_t, size_t);
typedef void (*release_fn)(uint8_t *, size_t);
enum {
  TEXT,
  ESC,
  CSI,
  ESC_SEQ,
  APC_HEAD,
  GLASS_HEAD,
  APC_BODY,
  GLASS_BODY,
  APC_OVER,
  APC_PASS,
  OSC_HEAD,
  OSC_BODY,
  OSC_OVER,
  OSC_PASS,
  DCS_HEAD,
  DCS_BODY,
  DCS_OVER,
  DCS_PASS,
  TMUX_HEAD,
  TMUX_BODY,
  TMUX_OVER
};
enum { OUT_TEXT = 0, OUT_APC = 1, OUT_GLASS = 2, OUT_SIXEL = 3, OUT_OSC = 4 };
typedef struct {
  int state, esc;
  uint8_t *buf;
  size_t len, capacity, body_cap;
  unsigned depth;
  int failed;
  grow_fn grow;
  release_fn release;
} splitter;
void *angel_terminal_splitter_new_v1(size_t cap, grow_fn grow,
                                     release_fn release) {
  if (!grow || !release)
    return NULL;
  splitter *s = calloc(1, sizeof(*s));
  if (s) {
    s->body_cap = cap;
    s->grow = grow;
    s->release = release;
  }
  return s;
}
void angel_terminal_splitter_free_v1(void *v) {
  splitter *s = v;
  if (s) {
    if (s->buf)
      s->release(s->buf, s->capacity);
    free(s);
  }
}
static void clear(splitter *s) {
  if (s->buf)
    s->release(s->buf, s->capacity);
  s->buf = NULL;
  s->len = s->capacity = 0;
}
static int reserve(splitter *s, size_t n) {
  if (n <= s->capacity)
    return 1;
  size_t v = s->capacity ? s->capacity : 8;
  while (v < n) {
    if (v > SIZE_MAX / 2) {
      v = n;
      break;
    }
    v *= 2;
  }
  void *p = s->grow(s->buf, s->capacity, v);
  if (!p) {
    s->failed = 1;
    return 0;
  }
  s->buf = p;
  s->capacity = v;
  return 1;
}
static void push(splitter *s, uint8_t b) {
  if (s->len == SIZE_MAX || !reserve(s, s->len + 1)) {
    s->failed = 1;
    return;
  }
  s->buf[s->len++] = b;
}
static void emit(splitter *s, emit_fn cb, void *ctx, int kind, const uint8_t *p,
                 size_t n, int borrowed) {
  if (!s->failed && !cb(ctx, kind, p, n, borrowed, 0))
    s->failed = 1;
}
/* The host allocator owns all buffers. A media callback takes ownership of
 * its buffer even if it returns failure; no pointer is retained afterward. */
static void emit_media(splitter *s, emit_fn cb, void *ctx, int kind) {
  uint8_t *p = s->buf;
  size_t n = s->len, cap = s->capacity;
  s->buf = NULL;
  s->len = s->capacity = 0;
  if (!s->failed) {
    if (!cb(ctx, kind, p, n, 0, cap))
      s->failed = 1;
  } else if (p)
    s->release(p, cap);
}
static void one(splitter *s, emit_fn cb, void *ctx, uint8_t a) {
  emit(s, cb, ctx, OUT_TEXT, &a, 1, 0);
}
static void pair(splitter *s, emit_fn cb, void *ctx, uint8_t a, uint8_t b) {
  uint8_t x[] = {a, b};
  emit(s, cb, ctx, OUT_TEXT, x, 2, 0);
}
static void prefixed(splitter *s, emit_fn cb, void *ctx, uint8_t intro,
                     int suffix) {
  size_t extra = 2 + (suffix >= 0);
  if (s->len > SIZE_MAX - extra || !reserve(s, s->len + extra)) {
    s->failed = 1;
    return;
  }
  memmove(s->buf + 2, s->buf, s->len);
  s->buf[0] = 27;
  s->buf[1] = intro;
  s->len += 2;
  if (suffix >= 0)
    s->buf[s->len++] = (uint8_t)suffix;
  emit(s, cb, ctx, OUT_TEXT, s->buf, s->len, 0);
  clear(s);
}
static void flush_run(splitter *s, emit_fn cb, void *ctx, const uint8_t *input,
                      size_t *run, size_t i, int borrowed) {
  if (*run != SIZE_MAX) {
    emit(s, cb, ctx, OUT_TEXT, input + *run, i - *run, borrowed);
    *run = SIZE_MAX;
  }
}
static int prefix(const uint8_t *p, size_t n, const char *s, size_t m) {
  return n >= m && !memcmp(p, s, m);
}
static int osc(const uint8_t *p, size_t n) {
  if (!n)
    return 1;
  const char *forms[] = {"1337;File=", "1337;FilePart=", "1337;FileEnd",
                         "1337;MultipartFile="};
  for (size_t i = 0; i < 4; i++) {
    size_t k = strlen(forms[i]);
    if (prefix(p, n, forms[i], k))
      return 2;
  }
  for (size_t i = 0; i < 4; i++) {
    size_t k = strlen(forms[i]);
    if (n <= k && !memcmp(p, forms[i], n))
      return 1;
  }
  return 0;
}
/* Return a complete, non-media escape's text length, or zero when the
 * streaming state machine must handle an incomplete or media sequence. */
static size_t whole_escape(const uint8_t *p, size_t n) {
  if (n < 2 || p[0] != 27) return 0;
  uint8_t b = p[1];
  if (b == '_' || b == ']' || b == 'P') return 0;
  if (b == 27) return 1;
  if (b == '[') {
    for (size_t j = 2; j < n; j++) {
      b = p[j];
      if (b >= 0x40 && b <= 0x7e) return j + 1;
      if (b == 27) return j;
      if (j - 1 > 256) return j + 1;
    }
    return 0;
  }
  if (b >= 0x20 && b <= 0x2f) {
    for (size_t j = 2; j < n; j++) {
      b = p[j];
      if (b >= 0x20 && b <= 0x2f) {
        if (j > 256) return j + 1;
        continue;
      }
      return b == 27 ? j : j + 1;
    }
    return 0;
  }
  return 2;
}
static void split_inner(splitter *s, const uint8_t *input, size_t n, emit_fn cb,
                        void *ctx, int borrowed) {
  size_t run = SIZE_MAX;
  for (size_t i = 0; i < n && !s->failed; i++) {
    if (s->state == TEXT && input[i] != 27) {
      if (run == SIZE_MAX)
        run = i;
      const uint8_t *next = memchr(input + i, 27, n - i);
      i = (next ? (size_t)(next - input) : n) - 1;
      continue;
    }
    if (s->state == TEXT && input[i] == 27) {
      size_t length = whole_escape(input + i, n - i);
      if (length) {
        flush_run(s, cb, ctx, input, &run, i, borrowed);
        emit(s, cb, ctx, OUT_TEXT, input + i, length, borrowed);
        i += length - 1;
        continue;
      }
    }
    if (!s->esc && (s->state == APC_BODY || s->state == GLASS_BODY ||
                    s->state == OSC_BODY || s->state == DCS_BODY || s->state == TMUX_BODY ||
                    s->state == TMUX_OVER || s->state == APC_OVER || s->state == OSC_OVER ||
                    s->state == DCS_OVER || s->state == APC_PASS ||
                    s->state == OSC_PASS || s->state == DCS_PASS)) {
      int st = s->state;
      int dcs = st == DCS_BODY || st == DCS_OVER || st == DCS_PASS ||
                st == TMUX_BODY || st == TMUX_OVER;
      size_t end = i;
      while (end < n && input[end] != 27 && input[end] != 0x18 &&
             input[end] != 0x1a && (dcs || input[end] != 7))
        end++;
      if (end > i) {
        if (st == APC_PASS || st == OSC_PASS || st == DCS_PASS) {
          if (run == SIZE_MAX) run = i;
        } else if (st == APC_BODY || st == GLASS_BODY || st == OSC_BODY || st == DCS_BODY || st == TMUX_BODY) {
          size_t room = s->len > s->body_cap ? 0 : s->body_cap - s->len;
          if (s->len <= s->body_cap && room != SIZE_MAX) room++;
          size_t count = end - i;
          size_t copy = count < room ? count : room;
          if (copy && reserve(s, s->len + copy)) {
            memcpy(s->buf + s->len, input + i, copy);
            s->len += copy;
          }
          if (count > room && !s->failed) {
            clear(s);
            s->state = st == OSC_BODY ? OSC_OVER : st == DCS_BODY ? DCS_OVER :
                       st == TMUX_BODY ? TMUX_OVER : APC_OVER;
          }
        }
        i = end - 1;
        continue;
      }
    }
    uint8_t b = input[i];
    int again = 1;
    while (again && !s->failed) {
      again = 0;
      switch (s->state) {
      case TEXT:
        if (b == 27) {
          flush_run(s, cb, ctx, input, &run, i, borrowed);
          s->state = ESC;
        } else if (run == SIZE_MAX)
          run = i;
        break;
      case ESC:
        if (b == '_')
          s->state = APC_HEAD;
        else if (b == ']')
          s->state = OSC_HEAD;
        else if (b == 'P')
          s->state = DCS_HEAD;
        else if (b == '[')
          s->state = CSI;
        else if (b == 27) {
          one(s, cb, ctx, 27);
          s->state = ESC;
        } else if (b >= 0x20 && b <= 0x2f) {
          push(s, b);
          s->state = ESC_SEQ;
        } else {
          pair(s, cb, ctx, 27, b);
          s->state = TEXT;
        }
        break;
      case CSI:
        push(s, b);
        if (b >= 0x40 && b <= 0x7e) {
          prefixed(s, cb, ctx, '[', -1);
          s->state = TEXT;
        } else if (b == 27) {
          s->len--;
          prefixed(s, cb, ctx, '[', -1);
          s->state = ESC;
        } else if (s->len > 256) {
          prefixed(s, cb, ctx, '[', -1);
          s->state = TEXT;
        }
        break;
      case ESC_SEQ:
        push(s, b);
        if (b >= 0x20 && b <= 0x2f) {
          if (s->len > 256) {
            if (reserve(s, s->len + 1)) {
              memmove(s->buf + 1, s->buf, s->len);
              s->buf[0] = 27;
              s->len++;
              emit(s, cb, ctx, OUT_TEXT, s->buf, s->len, 0);
              clear(s);
            }
            s->state = TEXT;
          }
          break;
        }
        if (b == 27) {
          s->len--; /* prefix with just ESC */
          if (reserve(s, s->len + 1)) {
            memmove(s->buf + 1, s->buf, s->len);
            s->buf[0] = 27;
            s->len++;
            emit(s, cb, ctx, OUT_TEXT, s->buf, s->len, 0);
            clear(s);
          }
          s->state = ESC;
        } else {
          if (reserve(s, s->len + 1)) {
            memmove(s->buf + 1, s->buf, s->len);
            s->buf[0] = 27;
            s->len++;
            emit(s, cb, ctx, OUT_TEXT, s->buf, s->len, 0);
            clear(s);
          }
          s->state = TEXT;
        }
        break;
      case APC_HEAD:
        if (b == 'G') {
          push(s, 'G');
          s->state = APC_BODY;
          s->esc = 0;
        } else if (b == 'g') {
          push(s, 'g');
          s->state = GLASS_HEAD;
        } else if (b == 27) {
          pair(s, cb, ctx, 27, '_');
          s->state = ESC;
        } else if (b == 0x18 || b == 0x1a) {
          pair(s, cb, ctx, 27, '_');
          one(s, cb, ctx, b);
          s->state = TEXT;
        } else {
          uint8_t x[] = {27, '_', b};
          emit(s, cb, ctx, OUT_TEXT, x, 3, 0);
          s->state = APC_PASS;
          s->esc = 0;
        }
        break;
      case GLASS_HEAD:
        if (b == 27 || b == 7 || b == 0x18 || b == 0x1a) {
          prefixed(s, cb, ctx, '_', -1);
          if (b == 27)
            s->state = ESC;
          else {
            one(s, cb, ctx, b);
            s->state = TEXT;
          }
        } else {
          push(s, b);
          if (prefix(s->buf, s->len, "glass;", 6)) {
            s->state = GLASS_BODY;
            s->esc = 0;
          } else if (s->len >= 6 || s->len > 256) {
            prefixed(s, cb, ctx, '_', -1);
            s->state = APC_PASS;
            s->esc = 0;
          }
        }
        break;
      case APC_BODY:
      case GLASS_BODY:
      case OSC_BODY:
      case DCS_BODY: {
        int st = s->state;
        if (s->esc) {
          if (b == '\\') {
            int k = st == APC_BODY     ? OUT_APC
                    : st == GLASS_BODY ? OUT_GLASS
                    : st == OSC_BODY   ? OUT_OSC
                                       : OUT_SIXEL;
            emit_media(s, cb, ctx, k);
            s->state = TEXT;
            s->esc = 0;
          } else {
            flush_run(s, cb, ctx, input, &run, i, borrowed);
            clear(s);
            s->state = ESC;
            s->esc = 0;
            again = 1;
          }
        } else if (b == 27)
          s->esc = 1;
        else if (b == 7 && st != DCS_BODY) {
          int k = st == APC_BODY     ? OUT_APC
                  : st == GLASS_BODY ? OUT_GLASS
                                     : OUT_OSC;
          emit_media(s, cb, ctx, k);
          s->state = TEXT;
        } else if (b == 0x18 || b == 0x1a) {
          clear(s);
          s->state = TEXT;
        } else if (s->len > s->body_cap) {
          clear(s);
          s->state = st == OSC_BODY   ? OSC_OVER
                     : st == DCS_BODY ? DCS_OVER
                                      : APC_OVER;
          s->esc = 0;
        } else
          push(s, b);
        break;
      }
      case APC_OVER:
      case OSC_OVER:
      case DCS_OVER: {
        int st = s->state;
        if (s->esc) {
          if (b == '\\') {
            s->state = TEXT;
            s->esc = 0;
          } else {
            flush_run(s, cb, ctx, input, &run, i, borrowed);
            s->state = ESC;
            s->esc = 0;
            again = 1;
          }
        } else if (b == 27)
          s->esc = 1;
        else if (b == 0x18 || b == 0x1a || (b == 7 && st != DCS_OVER))
          s->state = TEXT;
        break;
      }
      case APC_PASS:
      case OSC_PASS:
      case DCS_PASS: {
        int st = s->state;
        if (s->esc) {
          if (b == '\\') {
            pair(s, cb, ctx, 27, '\\');
            s->state = TEXT;
            s->esc = 0;
          } else if (b == 27) {
            pair(s, cb, ctx, 27, 27);
            s->esc = 0;
          } else {
            s->state = ESC;
            s->esc = 0;
            again = 1;
          }
        } else if (b == 27) {
          flush_run(s, cb, ctx, input, &run, i, borrowed);
          s->esc = 1;
        } else if (b == 0x18 || b == 0x1a || (b == 7 && st != DCS_PASS)) {
          if (run == SIZE_MAX)
            run = i;
          s->state = TEXT;
        } else if (run == SIZE_MAX)
          run = i;
        break;
      }
      case OSC_HEAD:
        if (b == 27) {
          prefixed(s, cb, ctx, ']', -1);
          s->state = ESC;
        } else if (b == 7 || b == 0x18 || b == 0x1a) {
          prefixed(s, cb, ctx, ']', b);
          s->state = TEXT;
        } else {
          push(s, b);
          int x = osc(s->buf, s->len);
          if (x == 2) {
            memmove(s->buf, s->buf + 5, s->len - 5);
            s->len -= 5;
            s->state = OSC_BODY;
            s->esc = 0;
          } else if (x == 0 || s->len > 256) {
            prefixed(s, cb, ctx, ']', -1);
            s->state = OSC_PASS;
            s->esc = 0;
          }
        }
        break;
      case DCS_HEAD:
        if (b >= 0x20 && b <= 0x3f) {
          push(s, b);
          if (s->len > 256) {
            prefixed(s, cb, ctx, 'P', -1);
            s->state = DCS_PASS;
            s->esc = 0;
          }
        } else if (b >= 0x40 && b <= 0x7e) {
          if (s->len == 0 && b == 't' && s->depth == 0) {
            push(s, 't');
            s->state = TMUX_HEAD;
          } else {
            int sixel = b == 'q';
            for (size_t j = 0; j < s->len; j++)
              if (s->buf[j] >= 0x20 && s->buf[j] <= 0x2f)
                sixel = 0;
            push(s, b);
            if (sixel) {
              s->state = DCS_BODY;
              s->esc = 0;
            } else {
              prefixed(s, cb, ctx, 'P', -1);
              s->state = DCS_PASS;
              s->esc = 0;
            }
          }
        } else if (b == 27) {
          prefixed(s, cb, ctx, 'P', -1);
          s->state = ESC;
        } else if (b == 7) {
          prefixed(s, cb, ctx, 'P', b);
          s->state = TEXT;
        } else if (b == 0x18 || b == 0x1a) {
          prefixed(s, cb, ctx, 'P', -1);
          one(s, cb, ctx, b);
          s->state = TEXT;
        } else {
          push(s, b);
          if (s->len > 256) {
            prefixed(s, cb, ctx, 'P', -1);
            s->state = DCS_PASS;
            s->esc = 0;
          }
        }
        break;
      case TMUX_HEAD:
        if (s->len < 5 && b == (uint8_t)"tmux;"[s->len]) {
          push(s, b);
          if (s->len == 5) {
            clear(s);
            s->state = TMUX_BODY;
            s->esc = 0;
          }
        } else {
          prefixed(s, cb, ctx, 'P', -1);
          s->state = DCS_PASS;
          s->esc = 0;
          again = 1;
        }
        break;
      case TMUX_BODY:
        if (s->esc) {
          if (b == 27) {
            if (s->len > s->body_cap) {
              clear(s);
              s->state = TMUX_OVER;
              s->esc = 0;
            } else {
              push(s, 27);
              s->esc = 0;
            }
          } else if (b == '\\') {
            uint8_t *body = s->buf;
            size_t len = s->len, capacity = s->capacity;
            s->buf = NULL;
            s->len = s->capacity = 0;
            s->state = TEXT;
            s->esc = 0;
            s->depth++;
            split_inner(s, body, len, cb, ctx, 0);
            s->depth--;
            if (body)
              s->release(body, capacity);
          } else if (b == 0x18 || b == 0x1a) {
            clear(s);
            s->state = TEXT;
            s->esc = 0;
          } else {
            clear(s);
            s->state = ESC;
            s->esc = 0;
            again = 1;
          }
        } else if (b == 27)
          s->esc = 1;
        else if (b == 0x18 || b == 0x1a) {
          clear(s);
          s->state = TEXT;
        } else if (s->len > s->body_cap) {
          clear(s);
          s->state = TMUX_OVER;
          s->esc = 0;
        } else
          push(s, b);
        break;
      case TMUX_OVER:
        if (s->esc) {
          if (b == '\\' || b == 0x18 || b == 0x1a) {
            s->state = TEXT;
            s->esc = 0;
          } else if (b == 27)
            s->esc = 0;
          else {
            s->state = ESC;
            s->esc = 0;
            again = 1;
          }
        } else if (b == 27)
          s->esc = 1;
        else if (b == 0x18 || b == 0x1a)
          s->state = TEXT;
        break;
      }
    }
  }
  flush_run(s, cb, ctx, input, &run, n, borrowed);
}
int angel_terminal_splitter_split_v1(void *v, const uint8_t *input, size_t n,
                                     emit_fn cb, void *ctx) {
  splitter *s = v;
  if (!s || (!input && n) || !cb)
    return 0;
  split_inner(s, input, n, cb, ctx, 1);
  return s->failed ? 0 : (s->state == TEXT ? 1 : 2);
}

uint32_t angel_terminal_splitter_abi_version(void) { return 1; }
