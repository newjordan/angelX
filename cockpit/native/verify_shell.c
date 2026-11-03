/* Shell verification classification. All spans and callbacks are borrowed only
 * for the duration of the call. Status-bearing mode fails closed for unclosed
 * quotes, substitutions, comments and heredocs; the Rust host handles the
 * separate `||` fast rejection. Returns 0 or 1, and -1 on allocation failure.
 */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
typedef int (*ws_fn)(uint32_t);
typedef struct {
  const unsigned char *p;
  size_t n;
} Span;
typedef struct {
  unsigned char *owner;
  Span span;
} Remote;
static inline int eqn(Span a, const char *b, size_t n) {
  if (a.n != n)
    return 0;
  for (size_t i = 0; i < n; i++) {
    unsigned char c = a.p[i], d = (unsigned char)b[i];
    if (c >= 'A' && c <= 'Z')
      c += 32;
    if (d >= 'A' && d <= 'Z')
      d += 32;
    if (c != d)
      return 0;
  }
  return 1;
}
#define eq(a, b) eqn((a), (b), sizeof(b) - 1)
static inline Span base(Span s, unsigned char sep) {
  for (size_t i = s.n; i; i--)
    if (s.p[i - 1] == sep) {
      s.p += i;
      s.n -= i;
      break;
    }
  return s;
}
static inline int ws(Span s, size_t i, size_t *step, ws_fn f) {
  unsigned char c = s.p[i];
  *step = 1;
  if (c < 128)
    return c == ' ' || (c >= 9 && c <= 13);
  size_t n = c < 0xe0 ? 2 : c < 0xf0 ? 3 : 4;
  uint32_t v = c & ((1u << (7 - n)) - 1);
  if (n > s.n - i)
    return 0;
  for (size_t k = 1; k < n; k++)
    v = (v << 6) | (s.p[i + k] & 63);
  *step = n;
  return f(v);
}
static int words(Span s, Span **out, size_t *num, ws_fn f, Span *stack,
                 size_t stackcap) {
  Span *v = stack;
  size_t len = 0, cap = stackcap, i = 0;
  while (i < s.n) {
    size_t k;
    while (i < s.n && ws(s, i, &k, f))
      i += k;
    if (i == s.n)
      break;
    size_t start = i;
    while (i < s.n && !ws(s, i, &k, f))
      i += k;
    if (len == cap) {
      size_t z = cap ? cap * 2 : 8;
      if (z < cap || z > SIZE_MAX / sizeof(*v)) {
        if (v != stack)
          free(v);
        return 0;
      }
      void *p;
      if (v == stack) {
        p = malloc(z * sizeof(*v));
        if (p)
          memcpy(p, v, len * sizeof(*v));
      } else
        p = realloc(v, z * sizeof(*v));
      if (!p) {
        if (v != stack)
          free(v);
        return 0;
      }
      v = p;
      cap = z;
    }
    v[len++] = (Span){s.p + start, i - start};
  }
  *out = v;
  *num = len;
  return 1;
}
static inline int assignment(Span s) {
  size_t i = 0;
  if (!s.n)
    return 0;
  unsigned char c = s.p[i++];
  if (c != '_' && !(c >= 'a' && c <= 'z') && !(c >= 'A' && c <= 'Z'))
    return 0;
  for (; i < s.n; i++) {
    c = s.p[i];
    if (c == '=')
      return 1;
    if (c != '_' && !(c >= 'a' && c <= 'z') && !(c >= 'A' && c <= 'Z') &&
        !(c >= '0' && c <= '9'))
      return 0;
  }
  return 0;
}
static int trim_nonempty(Span s, ws_fn f) {
  size_t k;
  for (size_t i = 0; i < s.n;) {
    if (!ws(s, i, &k, f))
      return 1;
    i += k;
  }
  return 0;
}
static int controls(Span s, int *async, int *ctrl, int *or) {
  int a = 0, b = 0, esc = 0;
  *async = *ctrl = *or = 0;
  for (size_t i = 0; i < s.n; i++) {
    unsigned char c = s.p[i], next = i + 1 < s.n ? s.p[i + 1] : 0,
                  prev = i ? s.p[i - 1] : 0;
    if (c == '|' && next == '|')
      *or = 1;
    if (esc) {
      esc = 0;
      continue;
    }
    if (c == '\\' && !a) {
      esc = 1;
      continue;
    }
    if (c == '\'' && !b) {
      a = !a;
      continue;
    }
    if (c == '"' && !a) {
      b = !b;
      continue;
    }
    if (c == '`' && !a)
      return 0;
    if (c == '$' && !a && next == '(')
      return 0;
    if (!a && !b) {
      if (c == '|' || c == '&' || c == ';' || c == '\n' || c == '\r')
        *ctrl = 1;
      if (((c == '<' || c == '>') && next == '(') || c == '(' || c == ')' ||
          c == '{' || c == '}' || (c == '<' && next == '<') || c == '#')
        return 0;
      if (c == '&' && prev != '&' && next != '&' && prev != '>' && next != '>')
        *async = 1;
    }
  }
  return !a && !b && !esc;
}
static int cmd(Span, ws_fn, int, int);
static int slow_iterative(Span, ws_fn);
static int seg(Span s, ws_fn f, int depth, int progress, Remote *next) {
  Span stack[32], *all = 0;
  size_t n = 0;
  if (!words(s, &all, &n, f, stack, 32))
    return -1;
  size_t start = 0;
  while (start < n && assignment(all[start]))
    start++;
  Span *v = all ? all + start : 0;
  n -= start;
  if (!n) {
    if (all != stack) {
      free(all);
    }
    return 0;
  }
  Span p = base(v[0], '/'), sub = n > 1 ? v[1] : (Span){0, 0};
  size_t off = 0;
  if (progress) {
    int r = 0;
#define EXACT(x, lit)                                                          \
  ((x).n == sizeof(lit) - 1 && memcmp((x).p, lit, sizeof(lit) - 1) == 0)
    if (EXACT(p, "cargo"))
      r = n > 1 && EXACT(v[1], "build");
    else if (EXACT(p, "npm") || EXACT(p, "pnpm") || EXACT(p, "yarn") ||
             EXACT(p, "bun"))
      r = n > 1 && (EXACT(v[1], "build") ||
                    (EXACT(v[1], "run") && n > 2 && EXACT(v[2], "build")));
    else if (EXACT(p, "cmake"))
      r = n > 1 && EXACT(v[1], "--build");
    else if (EXACT(p, "make") || EXACT(p, "ninja"))
      r = n == 1 || (n > 1 && (EXACT(v[1], "all") || EXACT(v[1], "build")));
    if (all != stack) {
      free(all);
    }
    return r;
  }
  if (eq(p, "cargo")) {
    int check = 0;
    for (size_t i = 0; i < n; i++)
      check |= eq(v[i], "--check");
    int r = n > 1 && (eq(v[1], "test") || eq(v[1], "check") ||
                      eq(v[1], "clippy") || (eq(v[1], "fmt") && check));
    if (all != stack) {
      free(all);
    }
    return r;
  }
  if ((eq(p, "npx") || eq(p, "bunx") || eq(p, "pnpx")))
    off = 1;
  else if ((eq(p, "npm") || eq(p, "pnpm") || eq(p, "yarn") || eq(p, "bun")) &&
           (eq(sub, "exec") || eq(sub, "dlx") || eq(sub, "x")))
    off = 2;
  else if ((eq(p, "pnpm") || eq(p, "yarn")))
    off = 1;
  if (off ||
      (eq(p, "jest") || eq(p, "vitest") || eq(p, "mocha") || eq(p, "ava") ||
       eq(p, "jasmine") || eq(p, "tap")) ||
      (v[0].n && v[0].p[0] == '-')) {
    for (size_t i = off; i < n; i++) {
      if (v[i].n && v[i].p[0] == '-')
        continue;
      Span r = base(v[i], '/');
      if ((eq(r, "jest") || eq(r, "vitest") || eq(r, "mocha") || eq(r, "ava") ||
           eq(r, "jasmine") || eq(r, "tap"))) {
        if (all != stack) {
          free(all);
        }
        return 1;
      }
      break;
    }
  }
  int result = 0;
  if (eq(p, "cargo")) {
    int check = 0;
    for (size_t i = 0; i < n; i++)
      check |= eq(v[i], "--check");
    result = n > 1 &&
             ((eq(v[1], "test") || eq(v[1], "check") || eq(v[1], "clippy")) ||
              (eq(v[1], "fmt") && check));
  } else if (eq(p, "node")) {
    for (size_t i = 1; i < n; i++)
      result |= eq(v[i], "--test");
  } else if ((eq(p, "npm") || eq(p, "pnpm") || eq(p, "yarn") || eq(p, "bun")))
    result = n > 1 && (eq(v[1], "test") ||
                       (eq(v[1], "run") && n > 2 && eq(v[2], "test")));
  else if ((eq(p, "pytest") || eq(p, "pytest-3") || eq(p, "ctest")))
    result = 1;
  else if ((eq(p, "python") || eq(p, "python3"))) {
    for (size_t i = 0; i + 1 < n; i++)
      result |= eq(v[i], "-m") &&
                (eq(v[i + 1], "pytest") || eq(v[i + 1], "unittest"));
  } else if (eq(p, "uv")) {
    if (n > 1 && eq(v[1], "run")) {
      result = n > 2 && (eq(v[2], "pytest") || eq(v[2], "pytest-3"));
      for (size_t i = 0; i + 2 < n; i++)
        result |= (eq(v[i], "python") || eq(v[i], "python3")) &&
                  eq(v[i + 1], "-m") && eq(v[i + 2], "pytest");
    }
  } else if (eq(p, "go"))
    result =
        n > 1 && (eq(v[1], "test") || eq(v[1], "build") || eq(v[1], "vet"));
  else if ((eq(p, "make") || eq(p, "just")))
    result = n > 1 && (eq(v[1], "test") || eq(v[1], "check"));
  else if ((eq(p, "mvn") || eq(p, "mvnw"))) {
    for (size_t i = 1; i < n; i++)
      result |= (eq(v[i], "test") || eq(v[i], "verify") || eq(v[i], "package"));
  } else if ((eq(p, "gradle") || eq(p, "gradlew"))) {
    for (size_t i = 1; i < n; i++)
      result |= (eq(base(v[i], ':'), "test") || eq(base(v[i], ':'), "check") ||
                 eq(base(v[i], ':'), "build"));
  } else if (eq(p, "dotnet"))
    result = n > 1 && (eq(v[1], "test") || eq(v[1], "build"));
  else if (eq(p, "ssh")) {
    size_t i = 1;
    while (i < n) {
      Span x = v[i];
      if (x.n && x.p[0] == '-') {
        int takes = x.n == 2 && x.p[1] != 0 && strchr("opilFcbEeJ", x.p[1]);
        i += takes ? 2 : 1;
      } else {
        i++;
        break;
      }
    }
    if (i < n) {
      size_t total = n - i - 1;
      for (size_t j = i; j < n; j++) {
        if (v[j].n > SIZE_MAX - total) {
          if (all != stack) {
            free(all);
          }
          return -1;
        }
        total += v[j].n;
      }
      unsigned char *b = malloc(total ? total : 1);
      if (!b) {
        if (all != stack) {
          free(all);
        }
        return -1;
      }
      size_t at = 0;
      for (size_t j = i; j < n; j++) {
        if (j > i)
          b[at++] = ' ';
        memcpy(b + at, v[j].p, v[j].n);
        at += v[j].n;
      }
      Span remote = {b, total};
      if (total >= 2 && ((b[0] == '\'' && b[total - 1] == '\'') ||
                         (b[0] == '"' && b[total - 1] == '"'))) {
        remote.p++;
        remote.n -= 2;
      }
      if (depth < 0) {
        next->owner = b;
        next->span = remote;
        if (all != stack) {
          free(all);
        }
        return -2;
      }
      result = depth >= 128 ? slow_iterative(remote, f)
                            : cmd(remote, f, 0, depth + 1);
      free(b);
    }
  }
  if (all != stack) {
    free(all);
  }
  return result;
}
static int any_once(Span s, ws_fn f, int depth) {
  size_t start = 0;
  int single = 0, dbl = 0, escaped = 0, found = 0, failed = 0;
  for (size_t i = 0; i < s.n; i++) {
    unsigned char c = s.p[i], next = i + 1 < s.n ? s.p[i + 1] : 0;
    if (escaped) {
      escaped = 0;
      continue;
    }
    if (c == '\\' && !single) {
      escaped = 1;
      continue;
    }
    if (c == '\'' && !dbl) {
      single = !single;
      continue;
    }
    if (c == '"' && !single) {
      dbl = !dbl;
      continue;
    }
    if (c == '`' && !single)
      return 0;
    if (c == '$' && !single && next == '(')
      return 0;
    if (!single && !dbl) {
      if (((c == '<' || c == '>') && next == '(') || c == '(' || c == ')' ||
          c == '{' || c == '}' || (c == '<' && next == '<') || c == '#')
        return 0;
      if (c == '|' || c == '&' || c == ';' || c == '\n' || c == '\r') {
        if (!found && !failed) {
          int r = seg((Span){s.p + start, i - start}, f, depth, 0, NULL);
          if (r < 0)
            failed = 1;
          else
            found = r;
        }
        start = i + 1;
      }
    }
  }
  if (single || dbl || escaped)
    return 0;
  if (found)
    return 1;
  if (failed)
    return -1;
  return seg((Span){s.p + start, s.n - start}, f, depth, 0, NULL);
}
static int cmd(Span s, ws_fn f, int anywhere, int depth) {
  if (anywhere == 1)
    return any_once(s, f, depth);
  int a = 0, ctl = 0, has_or = 0;
  if (!controls(s, &a, &ctl, &has_or))
    return 0;
  if (anywhere != 1 && (has_or || a))
    return 0;
  if (!ctl)
    return seg(s, f, depth, anywhere == 2, NULL);
  size_t start = 0, end = s.n;
  int q1 = 0, q2 = 0, esc = 0;
  if (anywhere != 1) {
    size_t last = 0;
    for (size_t i = 0; i < s.n; i++) {
      unsigned char c = s.p[i];
      if (esc) {
        esc = 0;
        continue;
      }
      if (c == '\\' && !q1) {
        esc = 1;
        continue;
      }
      if (c == '\'' && !q2) {
        q1 = !q1;
        continue;
      }
      if (c == '"' && !q1) {
        q2 = !q2;
        continue;
      }
      if (!q1 && !q2 && (c == ';' || c == '\n' || c == '\r')) {
        Span part = {s.p + last, i - last};
        if (trim_nonempty(part, f)) {
          start = last;
          end = i;
        }
        last = i + 1;
      }
    }
    Span tail = {s.p + last, s.n - last};
    if (trim_nonempty(tail, f)) {
      start = last;
      end = s.n;
    }
  }
  size_t at = start;
  q1 = q2 = esc = 0;
  for (size_t i = start; i <= end; i++) {
    unsigned char c = i < end ? s.p[i] : 0;
    if (i < end) {
      if (esc) {
        esc = 0;
        continue;
      }
      if (c == '\\' && !q1) {
        esc = 1;
        continue;
      }
      if (c == '\'' && !q2) {
        q1 = !q1;
        continue;
      }
      if (c == '"' && !q1) {
        q2 = !q2;
        continue;
      }
    }
    if (i == end ||
        (!q1 && !q2 &&
         (c == '|' || c == '&' ||
          (anywhere == 1 && (c == ';' || c == '\n' || c == '\r'))))) {
      int r = seg((Span){s.p + at, i - at}, f, depth, anywhere == 2, NULL);
      if (r)
        return r;
      at = i + 1;
    }
  }
  return 0;
}
typedef struct {
  unsigned char *owner;
  Span source;
  size_t next, end;
} Frame;
static int frame_init(Frame *frame, Remote r, ws_fn f) {
  Span s = r.span;
  int async = 0, ctl = 0, or = 0;
  if (!controls(s, &async, &ctl, &or) || async || or)
    return 0;
  size_t start = 0, end = s.n, last = 0;
  int single = 0, dbl = 0, escaped = 0;
  for (size_t i = 0; i < s.n; i++) {
    unsigned char c = s.p[i];
    if (escaped) {
      escaped = 0;
      continue;
    }
    if (c == '\\' && !single) {
      escaped = 1;
      continue;
    }
    if (c == '\'' && !dbl) {
      single = !single;
      continue;
    }
    if (c == '"' && !single) {
      dbl = !dbl;
      continue;
    }
    if (!single && !dbl && (c == ';' || c == '\n' || c == '\r')) {
      if (trim_nonempty((Span){s.p + last, i - last}, f)) {
        start = last;
        end = i;
      }
      last = i + 1;
    }
  }
  if (trim_nonempty((Span){s.p + last, s.n - last}, f)) {
    start = last;
    end = s.n;
  }
  *frame = (Frame){r.owner, s, start, end};
  return 1;
}
static int slow_iterative(Span root, ws_fn f) {
  size_t len = 0, cap = 16;
  Frame *stack = malloc(cap * sizeof(*stack));
  if (!stack)
    return -1;
  if (!frame_init(&stack[len], (Remote){NULL, root}, f)) {
    free(stack);
    return 0;
  }
  len++;
  int result = 0;
  while (len) {
    Frame *frame = &stack[len - 1];
    if (frame->next > frame->end) {
      free(frame->owner);
      len--;
      continue;
    }
    size_t start = frame->next, i = start;
    int single = 0, dbl = 0, escaped = 0;
    for (; i < frame->end; i++) {
      unsigned char c = frame->source.p[i];
      if (escaped) {
        escaped = 0;
        continue;
      }
      if (c == '\\' && !single) {
        escaped = 1;
        continue;
      }
      if (c == '\'' && !dbl) {
        single = !single;
        continue;
      }
      if (c == '"' && !single) {
        dbl = !dbl;
        continue;
      }
      if (!single && !dbl && (c == '|' || c == '&'))
        break;
    }
    frame->next = i + 1;
    Remote nested = {0};
    int r = seg((Span){frame->source.p + start, i - start}, f, -1, 0, &nested);
    if (r == 1) {
      result = 1;
      break;
    }
    if (r == -1) {
      result = -1;
      break;
    }
    if (r == -2) {
      if (len == cap) {
        size_t nc = cap * 2;
        if (nc < cap || nc > SIZE_MAX / sizeof(*stack)) {
          free(nested.owner);
          result = -1;
          break;
        }
        Frame *tmp = realloc(stack, nc * sizeof(*stack));
        if (!tmp) {
          free(nested.owner);
          result = -1;
          break;
        }
        stack = tmp;
        cap = nc;
      }
      if (frame_init(&stack[len], nested, f))
        len++;
      else
        free(nested.owner);
    }
  }
  while (len) {
    free(stack[--len].owner);
  }
  free(stack);
  return result;
}
int angel_verify_shell_v1(const unsigned char *s, size_t n, ws_fn f,
                          int anywhere) {
  if (!f || !s || anywhere < 0 || anywhere > 2)
    return -1;
  return cmd((Span){s, n}, f, anywhere, 0);
}

uint32_t angel_verify_shell_abi_version(void) { return 1; }
