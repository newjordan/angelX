/* Sixel raster decoder ABI v1. The caller supplies zeroed, byte-aligned
 * allocations and matching releases. On success the output allocation transfers
 * to the caller; on failure this function releases its intermediates. All
 * callbacks are synchronous. */
#include <limits.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#define CAP 8192u
#define REGS 1024u
typedef void *(*Alloc)(size_t);
typedef void (*Free)(void *, size_t);
typedef struct {
  uint8_t *p;
  size_t cols, rows;
  Alloc alloc;
  Free release;
  uint32_t regs[REGS], reg, num, den;
  size_t x, band, max_x, max_band;
  uint32_t ph, pv;
  int trans, failed;
} Dec;
typedef struct {
  uint8_t *p;
  size_t n, cap;
  uint32_t w, h;
} Image;
static const uint32_t defaults[16] = {0x000000, 0x3333cc, 0xcc2323, 0x33cc33,
                                      0xcc33cc, 0x33cccc, 0xcccc33, 0x7a7a7a,
                                      0x666666, 0x3333ff, 0xff3333, 0x33ff33,
                                      0xff33ff, 0x33ffff, 0xffff33, 0xffffff};
static size_t minz(size_t a, size_t b) { return a < b ? a : b; }
static size_t maxz(size_t a, size_t b) { return a > b ? a : b; }
static size_t pow2(size_t n) {
  size_t i = 1;
  while (i < n)
    i *= 2;
  return i;
}
static size_t rows(Dec *d, size_t bands) {
  uint64_t per = ((uint64_t)6 * d->num + d->den - 1) / d->den;
  uint64_t r = bands * per;
  return (size_t)(r > CAP ? CAP : r);
}
static void grow(Dec *d, size_t cols, size_t bands) {
  cols = minz(cols, CAP);
  bands = minz(bands, CAP);
  size_t need = rows(d, bands);
  if (cols <= d->cols && need <= d->rows)
    return;
  size_t nc = maxz(minz(pow2(maxz(cols, d->cols)), CAP), d->cols);
  size_t nr = maxz(minz(pow2(need), CAP), d->rows);
  if (nc <= d->cols && nr <= d->rows)
    return;
  uint8_t *p = d->alloc(nc * nr * 4);
  if (!p) {
    d->failed = 1;
    return;
  }
  for (size_t y = 0; y < d->rows; y++)
    memcpy(p + y * nc * 4, d->p + y * d->cols * 4, d->cols * 4);
  if (d->p)
    d->release(d->p, d->cols * d->rows * 4);
  d->p = p;
  d->cols = nc;
  d->rows = nr;
}
static void column(Dec *d, uint8_t code) {
  if (d->x >= CAP || d->band >= CAP || d->failed)
    return;
  grow(d, d->x + 1, d->band + 1);
  if (d->failed)
    return;
  uint32_t bits = (uint32_t)code - 0x3f, col = d->regs[d->reg];
  uint8_t px[4] = {(uint8_t)(col >> 16), (uint8_t)(col >> 8), (uint8_t)col,
                   255};
  uint64_t num = d->num, den = d->den;
  for (uint64_t bit = 0; bit < 6; bit++) {
    if (!(bits & (1u << bit)))
      continue;
    uint64_t sy = (uint64_t)d->band * 6 + bit;
    size_t y0 = minz((size_t)(sy * num / den), d->rows);
    size_t y1 = minz(maxz((size_t)((sy + 1) * num / den), y0 + 1), d->rows);
    for (size_t y = y0; y < y1; y++)
      memcpy(d->p + (y * d->cols + d->x) * 4, px, 4);
  }
  d->max_x = maxz(d->max_x, d->x);
  d->max_band = maxz(d->max_band, d->band);
  d->x++;
}
static uint32_t scale(uint32_t v) { return (v > 100 ? 100 : v) * 255 / 100; }
static uint32_t hls(uint32_t hue, uint32_t light, uint32_t sat) {
  float h = (float)((hue % 360) + 240);
  h = fmodf(h, 360.f);
  float l = (float)(light > 100 ? 100 : light) / 100.f,
        s = (float)(sat > 100 ? 100 : sat) / 100.f;
  float c = l <= .5f ? 2.f * l * s : (2.f - 2.f * l) * s;
  float hp = h / 60.f;
  float x = c * (1.f - fabsf(fmodf(hp, 2.f) - 1.f));
  float r = 0, g = 0, b = 0;
  switch ((uint32_t)(h / 60.f)) {
  case 0:
    r = c;
    g = x;
    break;
  case 1:
    r = x;
    g = c;
    break;
  case 2:
    g = c;
    b = x;
    break;
  case 3:
    g = x;
    b = c;
    break;
  case 4:
    r = x;
    b = c;
    break;
  default:
    r = c;
    b = x;
  }
  float m = l - c / 2.f;
  float rr = fminf(fmaxf(r + m, 0.f), 1.f), gg = fminf(fmaxf(g + m, 0.f), 1.f),
        bb = fminf(fmaxf(b + m, 0.f), 1.f);
  return ((uint32_t)roundf(rr * 255.f) << 16) |
         ((uint32_t)roundf(gg * 255.f) << 8) | (uint32_t)roundf(bb * 255.f);
}
static void colour(Dec *d, uint32_t *p, size_t n) {
  if (!n || p[0] >= REGS)
    return;
  uint32_t id = p[0];
  if (n >= 5 && p[1] == 2)
    d->regs[id] = scale(p[2]) << 16 | scale(p[3]) << 8 | scale(p[4]);
  else if (n >= 2 && p[1] == 3)
    d->regs[id] = hls(n > 2 ? p[2] : 0, n > 3 ? p[3] : 0, n > 4 ? p[4] : 0);
  d->reg = id;
}
static void raster(Dec *d, uint32_t *p, size_t n) {
  if (n >= 2 && p[0] && p[1]) {
    d->num = (uint32_t)minz(p[0], CAP);
    d->den = (uint32_t)minz(p[1], CAP);
  }
  if (n >= 4 && p[2] && p[3]) {
    d->ph = (uint32_t)minz(p[2], CAP);
    d->pv = (uint32_t)minz(p[3], CAP);
  }
}
static int finish(Dec *d, Image *out) {
  size_t h =
      d->rows ? minz((d->max_band + 1) * maxz(rows(d, 1), 1), d->rows) : 0;
  size_t w = d->cols ? minz(d->max_x + 1, d->cols) : 0;
  if (d->ph && d->pv) {
    w = minz(w, d->ph);
    h = minz(h, d->pv);
  }
  if (!w)
    w = 1;
  if (!h)
    h = maxz(rows(d, 1), 1);
  size_t size = w * h * 4;
  if (d->cols != w || d->rows < h) {
    uint8_t *p = d->alloc(w * h * 4);
    if (!p) {
      if (d->p)
        d->release(d->p, d->cols * d->rows * 4);
      return 0;
    }
    size_t cy = minz(d->rows, h), cx = minz(d->cols, w);
    for (size_t y = 0; y < cy; y++)
      memcpy(p + y * w * 4, d->p + y * d->cols * 4, cx * 4);
    if (d->p)
      d->release(d->p, d->cols * d->rows * 4);
    d->p = p;
    d->cols = w;
    d->rows = h;
  }
  if (!d->trans) {
    uint32_t c = d->regs[0];
    uint8_t px[4] = {(uint8_t)(c >> 16), (uint8_t)(c >> 8), (uint8_t)c, 255};
    for (size_t i = 0; i < size; i += 4)
      if (!d->p[i + 3])
        memcpy(d->p + i, px, 4);
  }
  *out = (Image){d->p, size, d->cols * d->rows * 4, (uint32_t)w, (uint32_t)h};
  return 1;
}
uint32_t angel_sixel_abi_version(void) { return 1; }
int angel_sixel_decode_v1(const uint8_t *data, size_t n, uint32_t first,
                          int trans, Alloc alloc, Free release, Image *out) {
  Dec d = {0};
  d.alloc = alloc;
  d.release = release;
  memcpy(d.regs, defaults, sizeof(defaults));
  d.num =
      first == 2
          ? 5
          : (first == 3 || first == 4 ? 3 : (first >= 7 && first <= 9 ? 1 : 2));
  d.den = 1;
  d.trans = trans;
  size_t i = 0;
  while (i < n && !d.failed) {
    uint8_t c = data[i];
    if (c == '!') {
      size_t j = i + 1;
      uint32_t count = 0;
      while (j < n && data[j] >= '0' && data[j] <= '9') {
        uint32_t x = data[j++] - '0';
        count = count > (UINT32_MAX - x) / 10 ? UINT32_MAX : count * 10 + x;
      }
      if (j < n) {
        size_t limit = minz(maxz(count, 1), CAP);
        for (size_t k = 0; k < limit; k++)
          column(&d, data[j]);
        i = j + 1;
      } else
        i = j;
    } else if (c == '#' || c == '"') {
      size_t j = i + 1, k = 0;
      uint32_t values[5] = {0}, v = 0;
      int has = 0, over = 0;
      while (j < n) {
        uint8_t a = data[j];
        if (a >= '0' && a <= '9') {
          uint32_t x = a - '0';
          if (v > (UINT32_MAX - x) / 10)
            over = 1;
          else
            v = v * 10 + x;
          has = 1;
          j++;
        } else if (a == ';') {
          if (has) {
            if (k < 5)
              values[k] = over ? 0 : v;
            k++;
            v = 0;
            has = over = 0;
          }
          j++;
        } else
          break;
      }
      if (has) {
        if (k < 5)
          values[k] = over ? 0 : v;
        k++;
      }
      if (c == '#')
        colour(&d, values, k);
      else
        raster(&d, values, k);
      i = j;
    } else if (c == '$') {
      d.x = 0;
      i++;
    } else if (c == '-') {
      if (d.band < CAP)
        d.band++;
      d.x = 0;
      i++;
    } else if (c == '+') {
      d.band = minz(d.band + 14, CAP);
      d.x = 0;
      i++;
    } else if (c >= 0x3f && c <= 0x7e) {
      column(&d, c);
      i++;
    } else
      i++;
  }
  if (d.failed) {
    if (d.p)
      d.release(d.p, d.cols * d.rows * 4);
    return 0;
  }
  return finish(&d, out);
}
