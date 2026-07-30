use "../strings.ox";
use "../event.ox";

fn discord_runtime_name() {
    return "Oxid Discord bot profile";
}

fn discord_runtime_note() {
    return "keep gateway handling, command dispatch, and cache logic in small Oxid modules";
}

fn discord_gateway_plan() {
    return join_lines([
        "gateway events",
        "slash commands",
        "presence updates",
        "message dispatch",
        "adapter isolation"
    ], ", ");
}

fn discord_bot_summary(bot_name, version, entry_point) {
    return join_lines([
        "discord bot: " + bot_name,
        "version: " + version,
        "entry: " + entry_point,
        "surface: gateway handlers, commands, and background tasks"
    ], "\n");
}

fn discord_bot_plan(bot_name, version, entry_point) {
    return join_lines([
        discord_bot_summary(bot_name, version, entry_point),
        event_runtime_note(),
        discord_gateway_plan()
    ], "\n");
}

fn discord_bot_command_list() {
    return join_lines([
        "slash commands",
        "prefix commands",
        "event listeners",
        "cache updates",
        "webhook adapters"
    ], ", ");
}
