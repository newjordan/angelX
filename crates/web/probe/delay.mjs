import net from 'node:net';

const [,, listen, target, oneWay = '75'] = process.argv;
if (!listen || !target) {
  console.error('usage: node delay.mjs <listen-port> <host:port> <one-way-ms>');
  process.exit(2);
}
const [host, port] = target.split(':');
const wait = Number(oneWay);

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
