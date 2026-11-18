#!/usr/bin/env python3
import asyncio
import os
import time

DELAY = float(os.environ.get("RELAY_DELAY_MS", "10")) / 1000.0
UP_HOST, UP_PORT = os.environ["RELAY_UPSTREAM"].rsplit(":", 1)
READY = os.environ["RELAY_LISTEN_READY"]
LOG = os.environ.get("RELAY_LOG")


async def pump(reader, writer):
    queue = asyncio.Queue()

    async def receive():
        while True:
            data = await reader.read(65536)
            queue.put_nowait((time.monotonic() + DELAY, data))
            if not data:
                return

    async def send():
        while True:
            due, data = await queue.get()
            wait = due - time.monotonic()
            if wait > 0:
                await asyncio.sleep(wait)
            if not data:
                try:
                    writer.write_eof()
                except (OSError, RuntimeError):
                    pass
                return
            writer.write(data)
            await writer.drain()

    try:
        await asyncio.gather(receive(), send())
    except (ConnectionError, OSError):
        pass


async def handle(client_reader, client_writer):
    if LOG:
        with open(LOG, "a") as log:
            log.write(f"{time.time():.6f} accept\n")
    up_reader, up_writer = await asyncio.open_connection(UP_HOST, int(UP_PORT))
    await asyncio.gather(pump(client_reader, up_writer), pump(up_reader, client_writer))
    for writer in (client_writer, up_writer):
        writer.close()


async def main():
    server = await asyncio.start_server(handle, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    with open(READY + ".tmp", "w") as ready:
        ready.write(f"{port}\n")
    os.replace(READY + ".tmp", READY)
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
