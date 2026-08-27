use "../stdlib/records.ox";
use "../stdlib/json.ox";

fn main() {
    let release = {
        language: "Oxid",
        version: 1,
        features: ["records", "json"],
        metadata: {stable: true}
    };
    record_put(release, "channel", "stable");

    let encoded = json_encode(release);
    let decoded = json_decode(encoded);
    assert(decoded.language == "Oxid", "JSON object property was not preserved");
    assert(decoded.metadata.stable, "nested JSON object was not preserved");
    assert(record_has(decoded, "channel"), "record mutation was not preserved");
    assert(json_encode(decoded) == encoded, "JSON roundtrip must be canonical");
    print encoded;
}
