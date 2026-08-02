# Web, Discord, and event-driven applications

Oxid includes executable application modules rather than documentation-only previews.

## Web

`stdlib/web.ox` provides route entries, method/path dispatch, text and JSON responses, and one-request TCP HTTP serving through the native runtime.

```bash
oxid web new my-api
cd my-api
oxid run src/main.ox
```

The module keeps routing and handler logic in Oxid. Production TLS, connection pooling, and deployment remain replaceable adapter concerns.

## Discord

`stdlib/bots/discord.ox` provides command registration, interaction dispatch, valid interaction response JSON, and a process adapter entry point.

```bash
oxid discord new my-bot
cd my-bot
oxid run src/main.ox
```

Set `DISCORD_TOKEN` for the generated profile. HTTPS/WebSocket gateway transport belongs in an adapter so that command logic remains independently testable in Oxid.

## Executable examples

- `examples/web_service.ox`
- `examples/discord_bot.ox`

Both examples are executed during full repository CI.
