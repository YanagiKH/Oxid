# Web, bots, and event-driven applications

Oxid is intended to be useful beyond scripts and compiler tooling. The language surface is being shaped so it can describe web services, Discord bots, and other event-driven systems with a small core and thin adapters.

## Design goals

- keep the app core event-driven
- keep I/O behind adapter modules
- keep async task orchestration simple
- keep the module surface readable for services and bots
- keep the native boundary small and replaceable

## Current module families

- `stdlib/event.ox` for service and event-loop patterns
- `stdlib/web.ox` for web-oriented service planning helpers
- `stdlib/bots/discord.ox` for Discord bot and gateway planning helpers

## Example entry points

- `examples/web_service.ox`
- `examples/discord_bot.ox`

## Intended usage model

The current repository keeps the runtime lightweight and uses Oxid modules to describe application structure, routing, handlers, command dispatch, and background tasks. Real network adapters can be supplied through the native layer or FFI-backed modules while the application logic stays in Oxid.

That means the same language can express:

- web APIs and JSON services
- Discord command bots
- background workers
- event-driven daemons
- queue consumers
- webhook handlers

## Recommended structure

```text
project/
├── src/
├── stdlib/
│   ├── event.ox
│   ├── web.ox
│   └── bots/
│       └── discord.ox
├── examples/
│   ├── web_service.ox
│   └── discord_bot.ox
└── docs/
```

## Practical note

Oxid is strongest when the service core is written in Oxid and platform-specific network details are isolated in a tiny adapter layer.
