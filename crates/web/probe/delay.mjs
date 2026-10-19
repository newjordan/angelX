// A link with a long way to go, for probes: every byte between a page and
// a host is held back a fixed time each way, so a host on this machine
// plays as one across an ocean does. The page and its socket both go
// through it.
//
//   node crates/web/probe/delay.mjs <listen-port> <host:port> <one-way-ms>
//
//   delve-dev-host --port 8820 --bundle crates/web/dist/delve --key K &
//   node crates/web/probe/delay.mjs 8821 127.0.0.1:8820 75 &
//   node crates/web/probe/probe.mjs 'http://127.0.0.1:8821/#K' --track own --fire
//
// 75 ms each way is a 150 ms round trip, about a transatlantic link. Order is kept (each chunk waits the same time, and Node
// runs equal timers in the order they were set). Node built-ins only.
import net from 'node:net';

const [,, listen, target, oneWay = '75'] = process.argv;
if (!listen || !target) {
  console.error('usage: node delay.mjs <listen-port> <host:port> <one-way-ms>');
  process.exit(2);
}
const [host, port] = target.split(':');
const wait = Number(oneWay);

// Bytes from `from` reach `to` `wait` ms later.
function pipe(from, to) {
  from.on('data', chunk => setTimeout(() => { if (!to.destroyed) to.write(chunk); }, wait));
  from.on('end', () => setTimeout(() => to.end(), wait));
  from.on('error', () => to.destroy());
}

net.createServer(page => {
  page.setNoDelay(true);
  const up = net.connect(Number(port), host, () => up.setNoDelay(true));
  pipe(page, up);
  pipe(up, page);
  page.on('close', () => setTimeout(() => up.destroy(), wait));
  up.on('close', () => setTimeout(() => page.destroy(), wait));
}).listen(Number(listen), '127.0.0.1', () =>
  console.log(`${wait} ms each way: http://127.0.0.1:${listen}/ -> ${target}`));
