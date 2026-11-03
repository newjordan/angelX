/* Versioned activity classifier for candidate Git inspection.
 * Input is valid UTF-8 and all spans and callbacks are borrowed only during
 * the synchronous call. The callback context is immutable and callbacks may
 * be called serially or concurrently. Modes: -1=all, 0=history, 1=grep,
 * 2=show. Returns 0 or 1; other values signal failure. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
typedef struct {
  const unsigned char *p;
  size_t n;
} Span;
typedef int (*pred)(const void *, int, const unsigned char *, size_t);
typedef int (*spacefn)(uint32_t);
static int eq(Span s, const char *v) {
  size_t n = strlen(v);
  return s.n == n && !memcmp(s.p, v, n);
}
static int prefix(Span s, const char *v) {
  size_t n = strlen(v);
  return s.n >= n && !memcmp(s.p, v, n);
}
static int has(Span s, const char *v) {
  size_t n = strlen(v);
  if (!n)
    return 1;
  if (n > s.n)
    return 0;
  const unsigned char *cur = s.p;
  size_t left = s.n - n + 1;
  while (left) {
    const unsigned char *hit = memchr(cur, (unsigned char)*v, left);
    if (!hit)
      return 0;
    if (!memcmp(hit, v, n))
      return 1;
    size_t step = (size_t)(hit - cur) + 1;
    cur += step;
    left -= step;
  }
  return 0;
}
static int any(Span s, const char *v) {
  for (size_t i = 0; i < s.n; i++)
    if (s.p[i] && strchr(v, s.p[i]))
      return 1;
  return 0;
}
static Span quote(Span s) {
  while (s.n && (*s.p == '\'' || *s.p == '"' || *s.p == '`')) {
    s.p++;
    s.n--;
  }
  while (s.n &&
         (s.p[s.n - 1] == '\'' || s.p[s.n - 1] == '"' || s.p[s.n - 1] == '`'))
    s.n--;
  return s;
}
static Span base(Span s) {
  for (size_t i = s.n; i; i--)
    if (s.p[i - 1] == '/') {
      s.p += i;
      s.n -= i;
      break;
    }
  return s;
}
static size_t rune(const unsigned char *p, size_t n, uint32_t *r) {
  unsigned char c = p[0];
  if (c < 128) {
    *r = c;
    return 1;
  }
  size_t k = c < 0xe0 ? 2 : c < 0xf0 ? 3 : 4;
  if (k > n) {
    *r = c;
    return 1;
  }
  uint32_t x = c & ((1u << (7 - k)) - 1);
  for (size_t i = 1; i < k; i++)
    x = (x << 6) | (p[i] & 63);
  *r = x;
  return k;
}
static int ws(uint32_t c, spacefn space) {
  return c < 128 ? (c == ' ' || c == '\t' || c == '\n' || c == '\r' ||
                    c == '\v' || c == '\f')
                 : space(c);
}
static Span next(Span s, size_t *at, spacefn space) {
  uint32_t r;
  size_t k;
  while (*at < s.n) {
    k = rune(s.p + *at, s.n - *at, &r);
    if (!ws(r, space))
      break;
    *at += k;
  }
  size_t a = *at;
  while (*at < s.n) {
    k = rune(s.p + *at, s.n - *at, &r);
    if (ws(r, space))
      break;
    *at += k;
  }
  return (Span){s.p + a, *at - a};
}
static int cb(pred p, const void *ctx, int mode, Span s) {
  return p(ctx, mode, s.p, s.n);
}
static int refs(Span s) {
  s = quote(s);
  if (eq(s, "HEAD") || eq(s, "ORIG_HEAD") || eq(s, "FETCH_HEAD") ||
      eq(s, "MERGE_HEAD") || eq(s, "main") || eq(s, "master") ||
      prefix(s, "refs/") || prefix(s, "origin/") || prefix(s, "upstream/") ||
      has(s, "..") || has(s, "@{"))
    return 1;
  if (s.n < 7 || s.n > 40)
    return 0;
  for (size_t i = 0; i < s.n; i++) {
    unsigned c = s.p[i];
    if (!((c >= '0' && c <= '9') || (c >= 'A' && c <= 'F') ||
          (c >= 'a' && c <= 'f')))
      return 0;
  }
  return 1;
}
static int flag_value(Span s) {
  const char *a[] = {"-e",
                     "-f",
                     "-O",
                     "-A",
                     "-B",
                     "-C",
                     "-j",
                     "--max-count",
                     "--max-depth",
                     "--threads",
                     "--open-files-in-pager"};
  for (size_t i = 0; i < sizeof a / sizeof *a; i++)
    if (eq(s, a[i]))
      return 1;
  return prefix(s, "--max-count=") || prefix(s, "--max-depth=") ||
         prefix(s, "--threads=") ||
         (prefix(s, "-") && !prefix(s, "--") && any(s, "efOABCj"));
}
static int flag_pattern(Span s) {
  return eq(s, "-e") || eq(s, "-f") ||
         (prefix(s, "-") && !prefix(s, "--") && any(s, "e"));
}
static int product_path(Span s, pred callback, const void *ctx) {
  Span original = s;
  s = quote(s);
  if (!s.n || s.p[0] == '-' || s.p[0] == '/' || s.p[0] == '\\')
    return 0;
  for (size_t i = 0; i < s.n; i++) {
    unsigned char c = s.p[i];
    if (c == '>' || c == '<' || c == '|' || c == '=')
      return 0;
  }
  int changed = s.p != original.p || s.n != original.n;
  if (s.n >= 2 && s.p[0] == '.' && (s.p[1] == '/' || s.p[1] == '\\')) {
    s.p += 2;
    s.n -= 2;
    changed = 1;
  }
  if (!s.n ||
      (changed && (cb(callback, ctx, 0, s) || cb(callback, ctx, 1, s))) ||
      prefix(s, "**") || prefix(s, "*.") || eq(s, "*"))
    return 0;
  return memchr(s.p, '/', s.n) != NULL || memchr(s.p, '\\', s.n) != NULL;
}
static int history_sub(Span s) {
  const char *a[] = {"log",         "blame",       "annotate",   "ls-files",
                     "ls-tree",     "shortlog",    "rev-list",   "reflog",
                     "whatchanged", "hash-object", "describe",   "archive",
                     "diff-tree",   "diff-index",  "diff-files", "check-attr",
                     "name-rev",    "check-ignore"};
  for (size_t i = 0; i < sizeof a / sizeof *a; i++)
    if (eq(s, a[i]))
      return 1;
  return 0;
}
static int inventory(Span s) {
  const char *a[] = {"--stat",       "--name-only", "--name-status",
                     "--numstat",    "--shortstat", "--dirstat",
                     "--summary",    "--raw",       "--batch",
                     "--batch-check"};
  if (has(s, " -- "))
    return 1;
  for (size_t i = 0; i + 1 < s.n; i++)
    if (s.p[i] == '-' && s.p[i + 1] == '-') {
      Span at = {s.p + i, s.n - i};
      for (size_t j = 0; j < sizeof a / sizeof *a; j++)
        if (prefix(at, a[j]))
          return 1;
    }
  return 0;
}
/* mode -1=auto, 0=history, 1=git grep, 2=git show. Caller trims source using
 * Rust. Callback modes: 0 board, 1 meta. */
int angel_activity_git_v1(const unsigned char *p, size_t n, int mode,
                          pred callback, const void *ctx, spacefn space) {
  int automatic = mode == -1;
  Span s = {p, n};
  if (!n || cb(callback, ctx, 0, s))
    return 0;
  for (size_t i = 0; i < n; i++) {
    unsigned char c = p[i];
    if (c == '|' || c == ';' || c == '`' || c == '\n' ||
        (c == '&' && i + 1 < n && p[i + 1] == '&'))
      return 0;
  }
  if (mode == 2 && inventory(s))
    return 0;
  size_t at = 0;
  Span t;
  do {
    t = next(s, &at, space);
    if (!t.n)
      return 0;
    if (!(has(t, "=") && t.p[0] != '-' && at < s.n))
      break;
  } while (1);
  if (!eq(base(t), "git"))
    return 0;
  Span sub = {0};
  int skip = 0, after = 0, pattern = 0, product = 0;
  while ((t = next(s, &at, space)).n) {
    t = quote(t);
    if (skip) {
      skip = 0;
      continue;
    }
    if (!sub.p) {
      if (eq(t, "-C") || eq(t, "--git-dir") || eq(t, "--work-tree")) {
        skip = 1;
        continue;
      }
      if (prefix(t, "-"))
        continue;
      sub = t;
      if (mode == -1) {
        if (history_sub(sub))
          mode = 0;
        else if (eq(sub, "grep"))
          mode = 1;
        else if (eq(sub, "show") || eq(sub, "cat-file") ||
                 eq(sub, "rev-parse")) {
          if (inventory(s))
            return 0;
          mode = 2;
        } else
          return 0;
      }
      continue;
    }
    if (mode == 0) {
      if (!t.n || eq(t, "--") || prefix(t, "-") || refs(t))
        continue;
      if (cb(callback, ctx, 0, t) || cb(callback, ctx, 1, t))
        return 0;
      if (product_path(t, callback, ctx))
        product = 1;
    } else if (mode == 1) {
      if (!t.n)
        continue;
      if (eq(t, "--")) {
        after = 1;
        continue;
      }
      if (!after && prefix(t, "-")) {
        if (flag_value(t))
          skip = !has(t, "=");
        if (flag_pattern(t))
          pattern = 1;
        continue;
      }
      if (!after && !pattern) {
        pattern = 1;
        continue;
      }
      if (cb(callback, ctx, 0, t) || cb(callback, ctx, 1, t))
        return 0;
      if (product_path(t, callback, ctx))
        product = 1;
    } else {
      if (!t.n || prefix(t, "-") || has(t, "://"))
        continue;
      size_t k = 0;
      while (k < t.n && t.p[k] != ':')
        k++;
      if (!k || k == t.n || k + 1 == t.n)
        continue;
      Span rev = {t.p, k};
      if (has(rev, "/"))
        continue;
      Span path = {t.p + k + 1, t.n - k - 1};
      if (cb(callback, ctx, 0, path) || cb(callback, ctx, 1, path))
        return 0;
      if (product_path(path, callback, ctx))
        product = 1;
    }
  }
  return product &&
         (automatic || (mode == 0   ? history_sub(sub)
                        : mode == 1 ? eq(sub, "grep")
                                    : (eq(sub, "show") || eq(sub, "cat-file") ||
                                       eq(sub, "rev-parse"))));
}

uint32_t angel_activity_classifier_abi_version(void) { return 1; }

/* Competition inventory classification; callbacks borrow no data after return. */

typedef struct {
  const unsigned char *p;
  size_t n;
} CiSpan;
typedef int (*CiClassify)(uint32_t, int);
typedef struct {
  CiClassify cb;
  uint32_t sw, sa;
  int hw, ha, vw, va;
} CiClass;
static int ci_eq(CiSpan a, const char *b) {
  size_t n = strlen(b);
  return a.n == n && (!n || !memcmp(a.p, b, n));
}
static int ci_pref(CiSpan a, const char *b) {
  size_t n = strlen(b);
  return a.n >= n && (!n || !memcmp(a.p, b, n));
}
static int ci_has(CiSpan a, const char *b) {
  size_t n = strlen(b);
  if (!n)
    return 1;
  if (n > a.n)
    return 0;
  const unsigned char *p = a.p;
  size_t l = a.n - n + 1;
  while (l) {
    const unsigned char *h = memchr(p, *b, l);
    if (!h)
      return 0;
    if (!memcmp(h, b, n))
      return 1;
    size_t d = h - p + 1;
    p += d;
    l -= d;
  }
  return 0;
}
static uint32_t ci_rune(CiSpan s, size_t i, size_t *n) {
  unsigned char c = s.p[i];
  if (c < 128) {
    *n = 1;
    return c;
  }
  size_t k = c < 224 ? 2 : c < 240 ? 3 : 4;
  uint32_t u = c & ((1u << (7 - k)) - 1);
  for (size_t j = 1; j < k; j++)
    u = (u << 6) | (s.p[i + j] & 63);
  *n = k;
  return u;
}
static int ci_space(uint32_t x, CiClass *c) {
  if (x < 128)
    return x == ' ' || x == '\t' || x == '\n' || x == '\r' || x == '\v' ||
           x == '\f';
  if (!c->hw || c->sw != x) {
    c->sw = x;
    c->hw = 1;
    c->vw = c->cb(x, 0);
  }
  return c->vw;
}
static int ci_alnum(uint32_t x, CiClass *c) {
  if (x < 128)
    return (x >= 'a' && x <= 'z') || (x >= 'A' && x <= 'Z') ||
           (x >= '0' && x <= '9');
  if (!c->ha || c->sa != x) {
    c->sa = x;
    c->ha = 1;
    c->va = c->cb(x, 1);
  }
  return c->va;
}
static int ci_eqbyte(uint64_t v, unsigned b) {
  uint64_t t = v ^ (UINT64_C(0x0101010101010101) * b);
  return ((t - UINT64_C(0x0101010101010101)) & ~t &
          UINT64_C(0x8080808080808080)) != 0;
}
static void ci_nonspace(CiSpan s, size_t *at, CiClass *c, int pipe) {
  size_t k;
  while (s.n - *at >= 8) {
    uint64_t v;
    memcpy(&v, s.p + *at, 8);
    if ((v & UINT64_C(0x8080808080808080)) || ci_eqbyte(v, ' ') ||
        ci_eqbyte(v, 9) || ci_eqbyte(v, 10) || ci_eqbyte(v, 11) ||
        ci_eqbyte(v, 12) || ci_eqbyte(v, 13) || (pipe && ci_eqbyte(v, '|')))
      break;
    *at += 8;
  }
  while (*at < s.n) {
    size_t start = *at;
    uint32_t x = ci_rune(s, *at, &k);
    if (ci_space(x, c) || (pipe && x == '|'))
      break;
    *at += k;
    // Repeated non-ASCII scalars are common in large generated commands.
    // They share one classification and can be skipped in aligned blocks.
    if (k > 1 && s.n - *at >= k * 4 && !memcmp(s.p + start, s.p + *at, k)) {
      unsigned char pattern[16];
      size_t block = k * (sizeof(pattern) / k);
      for (size_t i = 0; i < block; i += k)
        memcpy(pattern + i, s.p + start, k);
      while (s.n - *at >= block && !memcmp(s.p + *at, pattern, block))
        *at += block;
    }
  }
}
static CiSpan ci_next(CiSpan s, size_t *at, CiClass *c) {
  size_t k;
  while (*at < s.n) {
    uint32_t x = ci_rune(s, *at, &k);
    if (!ci_space(x, c))
      break;
    *at += k;
  }
  size_t b = *at;
  ci_nonspace(s, at, c, 0);
  return (CiSpan){s.p + b, *at - b};
}
static CiSpan ci_base(CiSpan s) {
  for (size_t i = s.n; i; i--)
    if (s.p[i - 1] == '/') {
      s.p += i;
      s.n -= i;
      break;
    }
  return s;
}
static int ci_sep(uint32_t x, CiClass *c) {
  return ci_space(x, c) ||
         (x < 128 && x && strchr("/\\|&;()[]{}<>`\"'=,:", (int)x));
}
static int ci_eqfold(CiSpan s, const char *t) {
  size_t n = strlen(t);
  if (s.n != n)
    return 0;
  for (size_t i = 0; i < n; i++) {
    unsigned a = s.p[i], b = (unsigned char)t[i];
    if (a >= 'A' && a <= 'Z')
      a += 32;
    if (b >= 'A' && b <= 'Z')
      b += 32;
    if (a != b)
      return 0;
  }
  return 1;
}
static int ci_token(CiSpan s, const char *t, CiClass *c) {
  size_t i = 0;
  while (i < s.n) {
    size_t k;
    uint32_t x = ci_rune(s, i, &k);
    if (ci_sep(x, c)) {
      i += k;
      continue;
    }
    size_t b = i, last = i;
    while (i < s.n) {
      x = ci_rune(s, i, &k);
      if (ci_sep(x, c))
        break;
      if (ci_alnum(x, c) || x == '-' || x == '_')
        last = i + k;
      i += k;
    }
    while (b < last) {
      x = ci_rune(s, b, &k);
      if (ci_alnum(x, c) || x == '-' || x == '_')
        break;
      b += k;
    }
    CiSpan w = {s.p + b, last - b};
    if (ci_eqfold(w, t) ||
        (ci_pref(w, "--") && ci_eqfold((CiSpan){w.p + 2, w.n - 2}, t)))
      return 1;
  }
  return 0;
}
static int ci_ls(CiSpan s, CiClass *c) {
  int saw = 0;
  size_t at = 0;
  while (at < s.n) {
    CiSpan x = ci_base(ci_next(s, &at, c));
    if (!x.n)
      continue;
    if (ci_has(x, "=") && x.p[0] != '-')
      continue;
    if (x.p[0] != '-') {
      if (ci_eq(x, "ls"))
        saw = 1;
      continue;
    }
    if (!saw)
      continue;
    if (ci_pref(x, "--")) {
      if (ci_eq(x, "--recursive") || ci_eq(x, "--recurse") ||
          ci_pref(x, "--recursive=") || ci_pref(x, "--recurse="))
        return 1;
    } else
      for (size_t i = 1; i < x.n; i++)
        if (x.p[i] == 'r')
          return 1;
  }
  return 0;
}
static int ci_contains_fold(CiSpan s, const char *term) {
  size_t n = strlen(term);
  if (s.n < n)
    return 0;
  const unsigned char *p = s.p;
  size_t left = s.n - n + 1;
  unsigned char lower = term[0],
                upper = lower >= 'a' && lower <= 'z' ? lower - 32 : lower;
  while (left) {
    const unsigned char *a = memchr(p, lower, left),
                        *b = memchr(p, upper, left);
    const unsigned char *h = (!a ? b : (!b || a < b ? a : b));
    if (!h)
      return 0;
    size_t j = 1;
    while (j < n) {
      unsigned x = h[j];
      if (x >= 'A' && x <= 'Z')
        x += 32;
      if (x != (unsigned char)term[j])
        break;
      j++;
    }
    if (j == n)
      return 1;
    size_t step = h - p + 1;
    p += step;
    left -= step;
  }
  return 0;
}
static int ci_inventory_substring(CiSpan s) {
  for (size_t i = 0; i < s.n; i++) {
    CiSpan x = {s.p + i, s.n - i};
    switch (*x.p) {
    case 'f':
      if (ci_pref(x, "find ") || ci_pref(x, "find\t") || ci_pref(x, "find/") ||
          ci_pref(x, "fgrep ") || ci_pref(x, "fd ") || ci_pref(x, "fd\t"))
        return 1;
      break;
    case 'r':
      if (ci_pref(x, "rg ") || ci_pref(x, "rg\t") || ci_pref(x, "ripgrep ") ||
          ci_pref(x, "ripgrep\t"))
        return 1;
      break;
    case 'g':
      if (ci_pref(x, "grep ") || ci_pref(x, "grep\t") ||
          ci_pref(x, "git grep") || ci_pref(x, "git ls-files") ||
          ci_pref(x, "git ls-tree") || ci_pref(x, "git blame") ||
          ci_pref(x, "git log") || ci_pref(x, "git diff") ||
          ci_pref(x, "git status") || ci_pref(x, "git shortlog") ||
          ci_pref(x, "git rev-list") || ci_pref(x, "git reflog") ||
          ci_pref(x, "git whatchanged"))
        return 1;
      break;
    case 'e':
      if (ci_pref(x, "egrep "))
        return 1;
      break;
    case 'a':
      if (ci_pref(x, "ag ") || ci_pref(x, "ag\t") || ci_pref(x, "ack "))
        return 1;
      break;
    case 't':
      if (ci_pref(x, "tree "))
        return 1;
      break;
    case 'l':
      if (ci_pref(x, "locate "))
        return 1;
      break;
    case 'u':
      if (ci_pref(x, "ugrep ") || ci_pref(x, "ugrep\t"))
        return 1;
      break;
    }
  }
  return 0;
}
static int ci_show(CiSpan s, CiClass *c) {
  if (!ci_contains_fold(s, "show") || !ci_contains_fold(s, "git") ||
      !ci_token(s, "show", c) || !ci_token(s, "git", c))
    return 0;
  const char *flags[] = {"--stat",    "--name-only", "--name-status",
                         "--numstat", "--shortstat", "--dirstat",
                         "--summary", "--raw",       " -- "};
  for (size_t i = 0; i < sizeof(flags) / sizeof(*flags); i++)
    if (ci_has(s, flags[i]))
      return 1;
  size_t at = 0;
  while (at < s.n) {
    CiSpan x = ci_next(s, &at, c);
    if (!x.n || x.p[0] == '-')
      continue;
    const unsigned char *p = memchr(x.p, ':', x.n);
    if (p && p > x.p && p < x.p + x.n - 1 && !memchr(x.p, '/', p - x.p) &&
        !ci_has(x, "://"))
      return 0;
  }
  return 1;
}
int angel_activity_competition_v1(const unsigned char *p, size_t n, int mode,
                                  CiClassify callback) {
  CiClass storage = {.cb = callback};
  CiClass *c = &storage;
  CiSpan s = {p, n};
  if (mode == 1)
    return ci_ls(s, c);
  if (mode != 0)
    return -1;
  size_t k;
  while (s.n) {
    uint32_t r = ci_rune(s, 0, &k);
    if (!ci_space(r, c))
      break;
    s.p += k;
    s.n -= k;
  }
  while (s.n) {
    size_t i = 0;
    ci_nonspace(s, &i, c, 0);
    if (i == s.n)
      break;
    CiSpan h = {s.p, i};
    if (!ci_has(h, "=") || h.p[0] == '-')
      break;
    s.p += i;
    s.n -= i;
    while (s.n) {
      uint32_t r = ci_rune(s, 0, &k);
      if (!ci_space(r, c))
        break;
      s.p += k;
      s.n -= k;
    }
  }
  if (ci_inventory_substring(s))
    return 1;
  if (ci_show(s, c) || (ci_has(s, "ls") && ci_ls(s, c)))
    return 1;
  size_t i = 0;
  CiSpan first = {s.p, 0};
  while (i < s.n) {
    uint32_t x = ci_rune(s, i, &k);
    if (ci_space(x, c) || x == '|') {
      i += k;
      continue;
    }
    size_t b = i;
    ci_nonspace(s, &i, c, 1);
    CiSpan a = {s.p + b, i - b};
    if (a.n && !ci_has(a, "=")) {
      first = ci_base(a);
      break;
    }
  }
  const char *starts[] = {"find",  "rg", "ripgrep", "grep", "egrep", "fgrep",
                          "ugrep", "ag", "ack",     "fd",   "tree",  "locate"};
  for (size_t x = 0; x < sizeof(starts) / sizeof(*starts); x++)
    if (ci_eq(first, starts[x]))
      return 1;
  if (!ci_eq(first, "git"))
    return 0;
  if (ci_has(s, "grep") || ci_has(s, "ls-files") || ci_has(s, "ls-tree"))
    return 1;
  const char *ts[] = {"blame",    "log",      "diff",   "status",
                      "shortlog", "rev-list", "reflog", "whatchanged"};
  for (size_t j = 0; j < sizeof(ts) / sizeof(*ts); j++)
    if (ci_contains_fold(s, ts[j]) && ci_token(s, ts[j], c))
      return 1;
  return 0;
}
