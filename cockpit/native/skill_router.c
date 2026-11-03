/* Deterministic skill selection. ABI v1 borrows every span and callback only
 * for the duration of a call, and returns SIZE_MAX for no match or
 * SIZE_MAX-1 on allocation failure. The host supplies Rust's Unicode
 * alphanumeric and whitespace classification. The library is reentrant. */
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
typedef struct {
  const char *p;
  size_t n;
} span;
typedef struct {
  span name, description;
} skill;
typedef int (*unicode_test)(uint32_t);
typedef struct {
  span raw, canon;
} pair;
typedef struct {
  char *copy;
  pair *pairs;
  size_t n;
} tokens;
static int cmp(span a, span b) {
  size_t n = a.n < b.n ? a.n : b.n;
  int v = memcmp(a.p, b.p, n);
  return v ? v : (a.n > b.n) - (a.n < b.n);
}
static int cmp_span(const void *a, const void *b) {
  return cmp(*(const span *)a, *(const span *)b);
}
static int cmp_len(const void *a, const void *b) {
  span x = *(const span *)a, y = *(const span *)b;
  if (x.n != y.n)
    return (x.n > y.n) - (x.n < y.n);
  return cmp(x, y);
}
static int eq(span a, const char *s) {
  size_t n = strlen(s);
  return a.n == n && !memcmp(a.p, s, n);
}
static int has(span a, const char *s) {
  size_t n = strlen(s);
  if (n > a.n)
    return 0;
  for (size_t i = 0; i <= a.n - n; i++)
    if (!memcmp(a.p + i, s, n))
      return 1;
  return 0;
}
static int starts(span a, span b) {
  return a.n >= b.n && !memcmp(a.p, b.p, b.n);
}
static int contains(span *set, size_t n, span key) {
  return bsearch(&key, set, n, sizeof(span), cmp_span) != NULL;
}
static uint32_t decode(const unsigned char *p, size_t n, size_t *used) {
  unsigned c = *p;
  if (c < 0x80) {
    *used = 1;
    return c;
  }
  size_t k = c < 0xe0 ? 2 : c < 0xf0 ? 3 : 4;
  if (k > n) {
    *used = 1;
    return c;
  }
  uint32_t v = c & ((1u << (7 - k)) - 1);
  for (size_t i = 1; i < k; i++)
    v = (v << 6) | (p[i] & 63);
  *used = k;
  return v;
}
static int alnum(uint32_t u, unicode_test cb) {
  if (u < 128)
    return (u >= 'a' && u <= 'z') || (u >= 'A' && u <= 'Z') ||
           (u >= '0' && u <= '9');
  return cb(u);
}
/* Constants in the switches below are FNV-1a hashes; every hit is checked
 * against its full token so collisions never change the classification. */
static uint32_t hash(span s) {
  uint32_t h = 2166136261u;
  for (size_t i = 0; i < s.n; i++)
    h = (h ^ (unsigned char)s.p[i]) * 16777619u;
  return h;
}
static span canonical(span raw) {
  switch (hash(raw)) {
  case 1943375221u:
    if (eq(raw, "bug"))
      return (span){"debug", 5};
    break;
  case 1596784754u:
    if (eq(raw, "bugs"))
      return (span){"debug", 5};
    break;
  case 2020464800u:
    if (eq(raw, "broken"))
      return (span){"debug", 5};
    break;
  case 441275864u:
    if (eq(raw, "crash"))
      return (span){"debug", 5};
    break;
  case 3406751772u:
    if (eq(raw, "crashes"))
      return (span){"debug", 5};
    break;
  case 4160821872u:
    if (eq(raw, "crashing"))
      return (span){"debug", 5};
    break;
  case 3121532249u:
    if (eq(raw, "crashed"))
      return (span){"debug", 5};
    break;
  case 563185489u:
    if (eq(raw, "error"))
      return (span){"debug", 5};
    break;
  case 4195859078u:
    if (eq(raw, "errors"))
      return (span){"debug", 5};
    break;
  case 508790637u:
    if (eq(raw, "fail"))
      return (span){"debug", 5};
    break;
  case 3682448442u:
    if (eq(raw, "fails"))
      return (span){"debug", 5};
    break;
  case 3769421748u:
    if (eq(raw, "failed"))
      return (span){"debug", 5};
    break;
  case 2920338483u:
    if (eq(raw, "failing"))
      return (span){"debug", 5};
    break;
  case 1113289521u:
    if (eq(raw, "failure"))
      return (span){"debug", 5};
    break;
  case 3086381286u:
    if (eq(raw, "failures"))
      return (span){"debug", 5};
    break;
  case 1612397662u:
    if (eq(raw, "debugger"))
      return (span){"debug", 5};
    break;
  case 2668789555u:
    if (eq(raw, "debugging"))
      return (span){"debug", 5};
    break;
  case 1529007906u:
    if (eq(raw, "tests"))
      return (span){"test", 4};
    break;
  case 3113626348u:
    if (eq(raw, "tested"))
      return (span){"test", 4};
    break;
  case 3948890523u:
    if (eq(raw, "testing"))
      return (span){"test", 4};
    break;
  case 2500064243u:
    if (eq(raw, "tdd"))
      return (span){"test", 4};
    break;
  case 2712230870u:
    if (eq(raw, "reviews"))
      return (span){"review", 6};
    break;
  case 3805047487u:
    if (eq(raw, "reviewing"))
      return (span){"review", 6};
    break;
  case 4116798120u:
    if (eq(raw, "audit"))
      return (span){"review", 6};
    break;
  case 1378521248u:
    if (eq(raw, "auditing"))
      return (span){"review", 6};
    break;
  case 1703357150u:
    if (eq(raw, "verification"))
      return (span){"verify", 6};
    break;
  case 2726738117u:
    if (eq(raw, "verified"))
      return (span){"verify", 6};
    break;
  case 1573505844u:
    if (eq(raw, "verifying"))
      return (span){"verify", 6};
    break;
  case 2631132491u:
    if (eq(raw, "validate"))
      return (span){"verify", 6};
    break;
  case 277989629u:
    if (eq(raw, "validated"))
      return (span){"verify", 6};
    break;
  case 3407426220u:
    if (eq(raw, "validating"))
      return (span){"verify", 6};
    break;
  case 2363230601u:
    if (eq(raw, "proof"))
      return (span){"verify", 6};
    break;
  case 2110036125u:
    if (eq(raw, "prove"))
      return (span){"verify", 6};
    break;
  case 752168611u:
    if (eq(raw, "navigation"))
      return (span){"navigate", 8};
    break;
  case 332581041u:
    if (eq(raw, "navigating"))
      return (span){"navigate", 8};
    break;
  case 3338526268u:
    if (eq(raw, "explore"))
      return (span){"navigate", 8};
    break;
  case 2449807919u:
    if (eq(raw, "exploring"))
      return (span){"navigate", 8};
    break;
  case 2142990227u:
    if (eq(raw, "refactor"))
      return (span){"simplify", 8};
    break;
  case 4094762400u:
    if (eq(raw, "refactors"))
      return (span){"simplify", 8};
    break;
  case 4120710981u:
    if (eq(raw, "refactoring"))
      return (span){"simplify", 8};
    break;
  case 3646812595u:
    if (eq(raw, "cleanup"))
      return (span){"simplify", 8};
    break;
  case 43574462u:
    if (eq(raw, "dedupe"))
      return (span){"simplify", 8};
    break;
  }
  return raw;
}
static int stop(span s) {
  switch (hash(s)) {
  case 3005452862u:
    return eq(s, "about");
  case 460225993u:
    return eq(s, "after");
  case 1340600742u:
    return eq(s, "agent");
  case 321211332u:
    return eq(s, "all");
  case 3469174610u:
    return eq(s, "also");
  case 254395046u:
    return eq(s, "and");
  case 740945997u:
    return eq(s, "any");
  case 1081029709u:
    return eq(s, "are");
  case 944111422u:
    return eq(s, "before");
  case 1624600460u:
    return eq(s, "but");
  case 4034148709u:
    return eq(s, "can");
  case 1922892221u:
    return eq(s, "change");
  case 992597834u:
    return eq(s, "changes");
  case 4180765940u:
    return eq(s, "code");
  case 442104839u:
    return eq(s, "coding");
  case 766920766u:
    return eq(s, "could");
  case 3631407781u:
    return eq(s, "data");
  case 387068906u:
    return eq(s, "development");
  case 551296432u:
    return eq(s, "does");
  case 4183046491u:
    return eq(s, "driven");
  case 343580968u:
    return eq(s, "each");
  case 2867484483u:
    return eq(s, "file");
  case 1055342736u:
    return eq(s, "files");
  case 2901640080u:
    return eq(s, "for");
  case 2513272949u:
    return eq(s, "from");
  case 3804167826u:
    return eq(s, "had");
  case 3988721635u:
    return eq(s, "has");
  case 4123919355u:
    return eq(s, "have");
  case 3038396907u:
    return eq(s, "harness");
  case 29600909u:
    return eq(s, "how");
  case 798074659u:
    return eq(s, "into");
  case 2163763161u:
    return eq(s, "its");
  case 2401607263u:
    return eq(s, "just");
  case 3600998790u:
    return eq(s, "may");
  case 2961925722u:
    return eq(s, "model");
  case 343003787u:
    return eq(s, "models");
  case 3897901536u:
    return eq(s, "more");
  case 4216529202u:
    return eq(s, "most");
  case 699505802u:
    return eq(s, "not");
  case 2644412919u:
    return eq(s, "only");
  case 3089275291u:
    return eq(s, "ops");
  case 2837066553u:
    return eq(s, "our");
  case 2870621791u:
    return eq(s, "out");
  case 838226447u:
    return eq(s, "over");
  case 1305840995u:
    return eq(s, "please");
  case 373935342u:
    return eq(s, "requesting");
  case 3317840938u:
    return eq(s, "should");
  case 3390427190u:
    return eq(s, "skill");
  case 4083227761u:
    return eq(s, "some");
  case 639763806u:
    return eq(s, "such");
  case 1226705564u:
    return eq(s, "system");
  case 3586313913u:
    return eq(s, "systematic");
  case 4098394220u:
    return eq(s, "task");
  case 3843578906u:
    return eq(s, "than");
  case 3407360812u:
    return eq(s, "that");
  case 3020861980u:
    return eq(s, "the");
  case 3422370919u:
    return eq(s, "their");
  case 3827492835u:
    return eq(s, "them");
  case 3844270454u:
    return eq(s, "then");
  case 3102463325u:
    return eq(s, "there");
  case 3572383752u:
    return eq(s, "these");
  case 3626161407u:
    return eq(s, "they");
  case 3660305025u:
    return eq(s, "this");
  case 4171928618u:
    return eq(s, "those");
  case 4254058618u:
    return eq(s, "through");
  case 1366625991u:
    return eq(s, "tool");
  case 4014351708u:
    return eq(s, "tools");
  case 2300132045u:
    return eq(s, "under");
  case 3979690785u:
    return eq(s, "very");
  case 383959538u:
    return eq(s, "was");
  case 3354087724u:
    return eq(s, "were");
  case 2238616623u:
    return eq(s, "what");
  case 2138539289u:
    return eq(s, "when");
  case 720644751u:
    return eq(s, "who");
  case 418647609u:
    return eq(s, "why");
  case 269512077u:
    return eq(s, "will");
  case 206241385u:
    return eq(s, "with");
  case 1536741984u:
    return eq(s, "work");
  case 3354971752u:
    return eq(s, "working");
  case 2059721058u:
    return eq(s, "would");
  case 1354289308u:
    return eq(s, "you");
  case 15787690u:
    return eq(s, "your");
  }
  return 0;
}
static int weak_corroboration(span s) {
  switch (hash(s)) {
  case 2221393911u:
    return eq(s, "already");
  case 653492337u:
    return eq(s, "done");
  case 1216469057u:
    return eq(s, "first");
  case 4200608216u:
    return eq(s, "good");
  case 18738364u:
    return eq(s, "green");
  case 3723446379u:
    return eq(s, "loop");
  case 3784070831u:
    return eq(s, "make");
  case 1555467752u:
    return eq(s, "next");
  case 681154065u:
    return eq(s, "new");
  case 3173751766u:
    return eq(s, "old");
  case 2071622424u:
    return eq(s, "pass");
  case 1362933657u:
    return eq(s, "passed");
  case 3041399728u:
    return eq(s, "passing");
  case 1089765596u:
    return eq(s, "red");
  case 3844270454u:
    return eq(s, "then");
  case 4105534985u:
    return eq(s, "well");
  }
  return 0;
}
static void token_free(tokens *t) {
  free(t->copy);
  free(t->pairs);
}
static int tokenize(span in, unicode_test cb, tokens *t) {
  memset(t, 0, sizeof(*t));
  size_t bytes = 0, count = 0;
  while (bytes < in.n && count < 4096) {
    size_t n;
    decode((const unsigned char *)in.p + bytes, in.n - bytes, &n);
    bytes += n;
    count++;
  }
  t->copy = malloc(bytes + 1);
  t->pairs = malloc((count + 1) * sizeof(pair));
  if (!t->copy || !t->pairs) {
    token_free(t);
    return 0;
  }
  memcpy(t->copy, in.p, bytes);
  t->copy[bytes] = 0;
  for (size_t i = 0; i < bytes; i++)
    if (t->copy[i] >= 'A' && t->copy[i] <= 'Z')
      t->copy[i] += 'a' - 'A';
  size_t i = 0;
  while (i < bytes) {
    size_t n;
    uint32_t u = decode((unsigned char *)t->copy + i, bytes - i, &n);
    if (!alnum(u, cb)) {
      i += n;
      continue;
    }
    size_t begin = i;
    i += n;
    while (i < bytes) {
      u = decode((unsigned char *)t->copy + i, bytes - i, &n);
      if (!alnum(u, cb))
        break;
      i += n;
    }
    span raw = {t->copy + begin, i - begin};
    if (raw.n < 3)
      continue;
    span canon = canonical(raw);
    if (canon.n < 3 || stop(canon))
      continue;
    t->pairs[t->n++] = (pair){raw, canon};
  }
  return 1;
}
static size_t set_of(tokens *t, span *out) {
  for (size_t i = 0; i < t->n; i++)
    out[i] = t->pairs[i].canon;
  qsort(out, t->n, sizeof(span), cmp_span);
  size_t n = 0;
  for (size_t i = 0; i < t->n; i++)
    if (!n || cmp(out[i], out[n - 1]))
      out[n++] = out[i];
  return n;
}
static span stem(span raw, span canon) {
  static const char *d[] = {"fail",    "fails",    "failed",   "failing",
                            "failure", "failures", "error",    "errors",
                            "broken",  "debugger", "debugging"};
  static const char *v[] = {"verification", "verified",  "verifying",
                            "validate",     "validated", "validating",
                            "proof",        "prove",     "verify"};
  static const char *r[] = {"review", "reviews", "reviewing", "audit",
                            "auditing"};
  static const char *s[] = {"refactor", "refactors", "refactoring", "cleanup",
                            "dedupe"};
  static const char *n[] = {"navigation", "navigating", "explore", "exploring",
                            "navigate"};
  const char **sets[] = {d, v, r, s, n};
  size_t counts[] = {11, 9, 5, 5, 5};
  const char *canons[] = {"debug", "verify", "review", "simplify", "navigate"};
  const char *result[] = {"weak_debug", "weak_verify", "weak_review",
                          "weak_simplify", "weak_navigate"};
  for (size_t i = 0; i < 5; i++)
    if (eq(canon, canons[i]))
      for (size_t j = 0; j < counts[i]; j++)
        if (eq(raw, sets[i][j]))
          return (span){result[i], strlen(result[i])};
  const char *suffix[] = {"ing", "ed", "es", "s"};
  for (size_t i = 0; i < 4; i++) {
    size_t z = strlen(suffix[i]);
    if (raw.n > z + 2 && !memcmp(raw.p + raw.n - z, suffix[i], z)) {
      raw.n -= z;
      break;
    }
  }
  return raw;
}
static size_t evidence(tokens *q, span *names, size_t nn, span *work) {
  size_t total = 0;
  for (size_t k = 0; k < nn; k++) {
    size_t ns = 0;
    for (size_t i = 0; i < q->n; i++)
      if (!cmp(q->pairs[i].canon, names[k]))
        work[ns++] = stem(q->pairs[i].raw, q->pairs[i].canon);
    qsort(work, ns, sizeof(span), cmp_span);
    size_t u = 0;
    for (size_t i = 0; i < ns; i++)
      if (!u || cmp(work[i], work[u - 1]))
        work[u++] = work[i];
    qsort(work, u, sizeof(span), cmp_len);
    size_t roots = 0;
    for (size_t i = 0; i < u; i++) {
      int same = 0;
      for (size_t j = 0; j < roots; j++)
        if (starts(work[i], work[j]) || starts(work[j], work[i])) {
          same = 1;
          break;
        }
      if (!same)
        work[roots++] = work[i];
    }
    total += roots;
  }
  return total;
}
static int ascii_alnum(unsigned char c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
         (c >= '0' && c <= '9');
}
static int explicit_slug(span task, span name) {
  int multi = 0;
  for (size_t i = 0; i < name.n; i++)
    if (name.p[i] == '-' || name.p[i] == '_' || name.p[i] == '.' ||
        name.p[i] == ':')
      multi = 1;
  if (!multi || name.n > task.n)
    return 0;
  for (size_t i = 0; i <= task.n - name.n; i++)
    if (!memcmp(task.p + i, name.p, name.n) &&
        (i == 0 || !ascii_alnum((unsigned char)task.p[i - 1])) &&
        (i + name.n == task.n ||
         !ascii_alnum((unsigned char)task.p[i + name.n])))
      return 1;
  return 0;
}
static int valid_name(span s) {
  if (!s.n || s.n > 64)
    return 0;
  for (size_t i = 0; i < s.n; i++) {
    unsigned c = s.p[i];
    if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
          (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.' ||
          c == ':'))
      return 0;
  }
  return 1;
}
static int research(span task, unicode_test space) {
  size_t i = 0;
  while (i < task.n) {
    size_t n;
    uint32_t u = decode((unsigned char *)task.p + i, task.n - i, &n);
    if (!space(u))
      break;
    i += n;
  }
  task.p += i;
  task.n -= i;
  for (size_t k = 0; k + 1 < task.n; k++)
    if (task.p[k] == '\n' && task.p[k + 1] == '\n') {
      task.n = k;
      break;
    }
  return has(task, "research question") || has(task, "answer this question") ||
         has(task, "answer the question") ||
         has(task, "answer using sources") ||
         has(task, "answer with citations");
}
size_t angel_skill_route_v1(span task, const skill *skills, size_t count,
                            unicode_test alnum_cb, unicode_test space_cb) {
  tokens q;
  if (!tokenize(task, alnum_cb, &q))
    return SIZE_MAX - 1;
  if (!q.n) {
    token_free(&q);
    return SIZE_MAX;
  }
  span *query = malloc(q.n * sizeof(span)), *work = malloc(q.n * sizeof(span));
  char *lower = malloc(task.n + 1);
  if (!query || !work || !lower) {
    free(query);
    free(work);
    free(lower);
    token_free(&q);
    return SIZE_MAX - 1;
  }
  memcpy(lower, task.p, task.n);
  lower[task.n] = 0;
  for (size_t i = 0; i < task.n; i++)
    if (lower[i] >= 'A' && lower[i] <= 'Z')
      lower[i] += 'a' - 'A';
  span task_lc = {lower, task.n};
  size_t qn = set_of(&q, query), best = SIZE_MAX, bs = 0, bm = 0, second = 0;
  int research_intent = research(task_lc, space_cb);
  for (size_t k = 0; k < count; k++) {
    span name = skills[k].name;
    if (!valid_name(name))
      continue;
    tokens nt;
    if (!tokenize(name, alnum_cb, &nt))
      goto fail;
    if (!nt.n) {
      token_free(&nt);
      continue;
    }
    span *ns = malloc(nt.n * sizeof(span));
    if (!ns) {
      token_free(&nt);
      goto fail;
    }
    size_t nn = set_of(&nt, ns), nm = 0;
    for (size_t i = 0; i < nn; i++)
      nm += contains(query, qn, ns[i]);
    if (!nm) {
      free(ns);
      token_free(&nt);
      continue;
    }
    size_t ev = evidence(&q, ns, nn, work);
    span slug = {nt.copy, name.n};
    int exact = explicit_slug(task_lc, slug);
    if (eq(slug, "research-answer") && !exact && !research_intent) {
      free(ns);
      token_free(&nt);
      continue;
    }
    tokens dt;
    if (!tokenize(skills[k].description, alnum_cb, &dt)) {
      free(ns);
      token_free(&nt);
      goto fail;
    }
    span *ds = malloc((dt.n + 1) * sizeof(span));
    if (!ds) {
      token_free(&dt);
      free(ns);
      token_free(&nt);
      goto fail;
    }
    size_t dn = set_of(&dt, ds), dm = 0, dc = 0;
    for (size_t i = 0; i < dn; i++)
      if (contains(query, qn, ds[i])) {
        dm++;
        if (!contains(ns, nn, ds[i]) && !weak_corroboration(ds[i]))
          dc++;
      }
    if (dm > 8)
      dm = 8;
    if (dc > 8)
      dc = 8;
    free(ds);
    token_free(&dt);
    free(ns);
    token_free(&nt);
    if (ev < 2 && !dc && !exact)
      continue;
    size_t score = nm * 8 + dm + (ev > 1 ? (ev - 1) * 2 : 0) + (exact ? 4 : 0);
    if (best == SIZE_MAX || score > bs ||
        (score == bs &&
         (nm > bm || (nm == bm && cmp(name, skills[best].name) < 0)))) {
      second = bs;
      best = k;
      bs = score;
      bm = nm;
    } else if (score > second)
      second = score;
  }
  free(query);
  free(work);
  free(lower);
  token_free(&q);
  return best != SIZE_MAX && bs >= 8 && (second == 0 || bs >= second + 4)
             ? best
             : SIZE_MAX;
fail:
  free(query);
  free(work);
  free(lower);
  token_free(&q);
  return SIZE_MAX - 1;
}

uint32_t angel_skill_route_abi_version(void) { return 1; }
