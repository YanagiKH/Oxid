//! Explicit native diagnosis, separate from historical project-only doctor.
use crate::{frontend, project_cli};

pub(crate) fn dispatch(args: &[String]) -> Option<i32> {
    let before_separator = args
        .iter()
        .position(|value| value == "--")
        .unwrap_or(args.len());
    let mut command = None;
    let mut index = 0;
    while index < before_separator {
        match args[index].as_str() {
            "--edition" | "--message-format" => index += 2,
            value if value.starts_with('-') => index += 1,
            _ => {
                command = Some(index);
                break;
            }
        }
    }
    let command = command?;
    if !matches!(args[command].as_str(), "doctor" | "diagnose")
        || !args[..before_separator]
            .iter()
            .any(|value| value == "--native" || value.starts_with("--native="))
    {
        return None;
    }
    let mut json = args.iter().any(|value| value == "--message-format=json")
        || args
            .windows(2)
            .any(|values| values == ["--message-format", "json"]);
    let result = (|| {
        let mut native = false;
        let mut edition = false;
        let mut format = false;
        let mut index = 0;
        while index < args.len() {
            if index == command {
                index += 1;
                continue;
            }
            let option = &args[index];
            if option == "--native" {
                if native {
                    return Err("--native may only be specified once");
                }
                native = true;
            } else {
                let (name, inline) = option
                    .split_once('=')
                    .map_or((option.as_str(), None), |(name, value)| (name, Some(value)));
                if !matches!(name, "--edition" | "--message-format") {
                    return Err("native doctor accepts only --native, --edition=typed-preview and --message-format=text|json");
                }
                let value = if let Some(value) = inline {
                    value
                } else {
                    index += 1;
                    args.get(index)
                        .map(String::as_str)
                        .ok_or("native doctor option is missing its value")?
                };
                match name {
                    "--edition" if !edition && value == "typed-preview" => edition = true,
                    "--message-format" if !format && matches!(value, "text" | "json") => {
                        format = true;
                        json = value == "json";
                    }
                    _ => return Err("duplicate or unsupported native doctor option value"),
                }
            }
            index += 1;
        }
        Ok(())
    })();
    if let Err(error) = result {
        return Some(frontend::NativeDoctorReport::invalid(error).print(json));
    }
    let mut report = frontend::native_doctor();
    report.project(project_cli::native_doctor_project());
    Some(report.print(json))
}
