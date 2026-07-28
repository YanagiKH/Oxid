use "../core.ox";

fn syntax_profile_name() {
    return "oxid-short";
}

fn syntax_feature_list() {
    return ["pub fn", "mod", "use as", "match", "try", "defer", "pipe"];
}

fn syntax_rule(feature) {
    if (feature == "pub fn") {
        return "exported declaration";
    }
    if (feature == "mod") {
        return "module grouping";
    }
    if (feature == "use as") {
        return "short alias import";
    }
    if (feature == "match") {
        return "pattern matching";
    }
    if (feature == "try") {
        return "error propagation";
    }
    if (feature == "defer") {
        return "cleanup hook";
    }
    if (feature == "pipe") {
        return "pipeline chaining";
    }
    return "syntax proposal";
}

fn syntax_summary() {
    let out = "";
    let items = syntax_feature_list();
    let i = 0;
    while (i < len(items)) {
        if (i > 0) {
            out = out + ", ";
        }
        out = out + items[i];
        i = i + 1;
    }
    return out;
}

fn syntax_note(feature) {
    return feature + " => " + syntax_rule(feature);
}
