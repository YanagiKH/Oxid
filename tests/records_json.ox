use "../stdlib/records.ox";
use "../stdlib/json.ox";

fn main() {
    let value = record_from_pairs(["z", 3, "a", 1]);
    assert(json_encode(value) == "{\"a\":1,\"z\":3}", "record keys must serialize canonically");

    record_put(value, "nested", {ready: true, items: [1, 2, 3]});
    assert(record_get(value, "missing", 42) == 42, "record fallback was ignored");
    assert(record_get(value, "nested", null).ready, "record_get lost a nested record");

    let selected = record_select(value, ["nested", "a"]);
    assert(json_encode(selected) == "{\"a\":1,\"nested\":{\"items\":[1,2,3],\"ready\":true}}",
        "record_select returned the wrong fields");

    let merged = record_merge_into({left: true}, {right: 2});
    assert(merged.left and merged.right == 2, "record_merge_into lost a field");

    let copied = json_clone(value);
    copied.nested.ready = false;
    assert(value.nested.ready, "json_clone must not alias the original record");
    assert(copied.nested.ready == false, "nested record assignment failed");

    assert(record_delete(value, "z") == 3, "record_delete returned the wrong value");
    assert(!record_has(value, "z"), "record_delete did not remove the key");
}
