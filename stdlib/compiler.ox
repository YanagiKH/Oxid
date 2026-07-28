use "strings.ox";
use "errors.ox";
use "package.ox";

fn stage_name(name) {
    return "[" + name + "]";
}

fn backend_name(name) {
    return "backend:" + name;
}

fn target_name(name, profile) {
    return name + ":" + profile;
}

fn compiler_banner(project_name, version) {
    return package_name("name = "" + project_name + ""\nversion = "" + version + ""\nentry = "src/main.ox"\n");
}

fn compile_plan(project_name, version, backend) {
    let out = "";
    out = out + stage_name("parse") + "\n";
    out = out + stage_name("check") + "\n";
    out = out + stage_name("lower") + "\n";
    out = out + backend_name(backend) + "\n";
    out = out + compiler_banner(project_name, version);
    return out;
}
