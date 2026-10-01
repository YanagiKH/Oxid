# Local HTTP services and Discord handlers

Use these APIs to experiment with local services and interaction logic. The 0.9 runtime has no concurrent task scheduler or built-in TLS; this example is a sequential local server.

## Persistent network adapter

`net_listen` creates a reusable nonblocking TCP listener. A server loop can call `net_accept(listener, timeout_ms)` repeatedly, or use `net_try_accept(listener)` to return a connection/`null` immediately during idle periods. It can read a bounded text chunk or structured HTTP request, write a response, close only that connection, and keep the listener alive.

```oxid
const server = net_listen("127.0.0.1", 8080);
loop yes {
    const client = net_try_accept(server);
    when client == none {
        sleep_ms(2);
        continue;
    }
    const request = http_read_request(client, 30000);
    const body = json_stringify({ok: yes, path: request.path});
    const response = {
        status: 200,
        headers: {"Content-Type": "application/json"},
        body: body
    };
    http_write_response(client, response, 30000);
    net_close(client);
}
```

Socket operations use finite limits and timeout-bounded polling. `net_try_accept` is the zero-wait primitive for idle-resilient application loops. HTTP parsing rejects conflicting framing, malformed lines, oversized headers/bodies, and unsafe response fields. `http_write_response` accepts a structured response record or a raw HTTP string, validates and encodes it, then performs the bounded write. The adapter does not provide TLS, authentication, rate limiting, or a concurrent event-loop scheduler. Production use needs a separately reviewed deployment and transport design. `net_read` validates each returned chunk as UTF-8; it is not a binary stream API or an incremental UTF-8 decoder.

`web_serve_once` remains available for one-request programs and accepts either an existing listener or a host/port pair.

## Web and Discord modules

`stdlib/web.ox` provides route entries, method/path dispatch, text/JSON response helpers, `web_try_accept`, and a forever loop that yields for 2 ms when no connection is ready. `stdlib/bots/discord.ox` keeps command registration and interaction dispatch separate from HTTPS/WebSocket gateway transport. Generate starter projects with `oxid web new <name>` and `oxid discord new <name>`.
