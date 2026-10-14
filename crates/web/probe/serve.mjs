// Test only: serves a built bundle the way angel-serve does ({{asset:NAME}}
// filled in, the same strict CSP), with no game server behind it, for the
// `?offline` test page. node probe/serve.mjs <dist-dir> [port]
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

const [,, dir, port = '8801'] = process.argv;
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.wasm': 'application/wasm', '.json': 'application/json',
  '.wav': 'audio/wav', '.ogg': 'audio/ogg', '.mp3': 'audio/mpeg', '.m4a': 'audio/mp4',
};
const CSP = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; " +
  "img-src 'self'; media-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; " +
  "frame-ancestors 'none'";

http.createServer((req, res) => {
  const url = new URL(req.url, 'http://local');
  const gzip = /\bgzip\b/.test(req.headers['accept-encoding'] || '');
  const send = (type, body, cache) => {
    const headers = { 'Content-Type': type, 'Content-Security-Policy': CSP, 'Cache-Control': cache };
    if (gzip) {
      headers['Content-Encoding'] = 'gzip';
      body = zlib.gzipSync(body);
    }
    res.writeHead(200, headers);
    res.end(body);
  };
  if (url.pathname === '/') {
    const html = fs.readFileSync(path.join(dir, 'index.html'), 'utf8')
      .replace(/\{\{asset:([A-Za-z0-9._-]+)\}\}/g, '/a/$1');
    return send(TYPES['.html'], Buffer.from(html), 'no-cache');
  }
  if (url.pathname.startsWith('/a/')) {
    const file = path.join(dir, path.basename(url.pathname));
    if (fs.existsSync(file)) return send(TYPES[path.extname(file)] || 'application/octet-stream', fs.readFileSync(file), 'no-cache');
  }
  res.writeHead(404);
  res.end();
}).listen(Number(port), '127.0.0.1', () => console.log(`serving ${dir} on http://127.0.0.1:${port}/`));
