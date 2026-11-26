//! Bounded registry search over a trusted HTTPS connection.
use super::{name, version, Budget, MAX_SEARCH_ENTRIES};
use crate::https;

pub(super) const HEADER: &str = "Oxid registry catalog 1\n";
const CATALOG_LIMIT: usize = 1024 * 1024;

fn parse(body: &[u8], prefix: &str, budget: &mut Budget) -> Result<Vec<(String, String)>, String> {
    let rows = std::str::from_utf8(body)
        .ok()
        .and_then(|text| text.strip_prefix(HEADER))
        .ok_or("unsupported registry catalog format")?;
    if !rows.is_empty() && !rows.ends_with('\n') {
        return Err("registry catalog has an incomplete final record".into());
    }
    let mut previous = "";
    let mut results = Vec::new();
    for (index, row) in rows.split_terminator('\n').enumerate() {
        if index >= MAX_SEARCH_ENTRIES {
            return Err("registry catalog exceeds the 4096-package limit".into());
        }
        let (package, latest) = row
            .split_once(' ')
            .ok_or("invalid registry catalog record")?;
        name(package)?;
        version(latest)?;
        if package <= previous {
            return Err("registry catalog package names must be unique and sorted".into());
        }
        previous = package;
        if package.starts_with(prefix) {
            budget.reserve(&mut results, 1)?;
            results.push((budget.text(package)?, budget.text(latest)?));
        }
    }
    Ok(results)
}

pub(super) fn search(
    url: &str,
    prefix: &str,
    variable: Option<&str>,
    budget: &mut Budget,
) -> Result<Vec<(String, String)>, String> {
    let mut body = Vec::new();
    budget.reserve(&mut body, CATALOG_LIMIT)?;
    https::download(url, variable, CATALOG_LIMIT as u64, 30, &mut body).map_err(
        |error| match error {
            https::Error::TooLarge => "registry catalog exceeds 1 MiB".into(),
            https::Error::Request(message) => message,
        },
    )?;
    parse(&body, prefix, budget)
}
