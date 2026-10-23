//! Immutable program arguments and the command-line boundary that owns them.
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    marker::PhantomData,
    rc::Rc,
};

#[derive(Default)]
struct Arguments {
    count: i32,
    values: Vec<OsString>,
}

thread_local! {
    static CURRENT: RefCell<Arguments> = RefCell::new(Arguments::default());
}

/// A scope cannot move to another thread because its destructor restores the
/// argument context of the thread that installed it.
pub(crate) struct Scope {
    previous: Arguments,
    _thread: PhantomData<Rc<()>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|current| {
            *current.borrow_mut() = std::mem::take(&mut self.previous);
        });
    }
}

pub(crate) fn install(values: Vec<OsString>) -> Result<Scope, &'static str> {
    let count = i32::try_from(values.len()).map_err(|_| "too many program arguments")?;
    let previous = CURRENT.with(|current| current.replace(Arguments { count, values }));
    Ok(Scope {
        previous,
        _thread: PhantomData,
    })
}

pub(crate) fn count() -> i32 {
    CURRENT.with(|current| current.borrow().count)
}

pub(crate) fn with_argument<R>(index: i32, use_bytes: impl FnOnce(Option<&[u8]>) -> R) -> R {
    CURRENT.with(|current| {
        let current = current.borrow();
        use_bytes(
            usize::try_from(index)
                .ok()
                .and_then(|index| current.values.get(index))
                .map(|value| value.as_encoded_bytes()),
        )
    })
}

type CopyResult = Result<(u32, Option<i32>), ()>;
struct CopyRequest<'a> {
    destination: &'a mut [u8],
    return_cost: usize,
    fuel: &'a mut usize,
}
impl CopyRequest<'_> {
    fn copy(self, argument: Option<&[u8]>) -> CopyResult {
        let (tag, payload, scan, copied) = match argument {
            None => (2, None, 0, 0),
            Some(bytes) => match i32::try_from(bytes.len()) {
                Err(_) => (3, None, i32::MAX as usize + 1, 0),
                Ok(length) if bytes.len() > self.destination.len() => {
                    (1, Some(length), bytes.len() + 1, 0)
                }
                Ok(length) => (0, Some(length), bytes.len() + 1, bytes.len()),
            },
        };
        let remaining = self
            .fuel
            .checked_sub(scan)
            .and_then(|remaining| remaining.checked_sub(copied))
            .and_then(|remaining| remaining.checked_sub(self.return_cost))
            .ok_or(())?;
        // Match the native NUL scan plus copy cost. The CLI already owns these
        // immutable input bytes; no allocation or fallible check follows here.
        *self.fuel = remaining;
        if copied != 0 {
            self.destination[..copied].copy_from_slice(&argument.unwrap()[..copied]);
        }
        Ok((tag, payload))
    }
}
pub(crate) fn copy_argument(
    index: i32,
    destination: &mut [u8],
    return_cost: usize,
    fuel: &mut usize,
) -> CopyResult {
    let request = CopyRequest {
        destination,
        return_cost,
        fuel,
    };
    with_argument(index, |argument| request.copy(argument))
}

// Complete fixed roles for the installed context, RefCell borrow, two nested
// callback captures, selected slice and returned status transports. Input OS
// strings belong to the CLI input boundary, not to the language resource heap.
#[allow(dead_code)]
struct ControlCarriers {
    context: RefCell<Arguments>,
    borrow: std::cell::Ref<'static, Arguments>,
    callbacks: [CopyRequest<'static>; 2],
    argument_views: [Option<&'static [u8]>; 3],
    statuses: [CopyResult; 3],
    indices: [i32; 3],
    lengths_and_costs: [usize; 6],
    tag: u32,
    payload: Option<i32>,
}
pub(crate) fn control_bytes() -> usize {
    std::mem::size_of::<ControlCarriers>()
}

pub(crate) struct Invocation {
    pub(crate) compiler: Vec<String>,
    original: Vec<OsString>,
    program: Vec<OsString>,
    separator: Option<usize>,
}

impl Invocation {
    pub(crate) fn parse(original: Vec<OsString>) -> Result<Self, &'static str> {
        let executable = original.first().cloned().unwrap_or_else(|| "oxid".into());
        let words = original.get(1..).unwrap_or_default();
        let split = program_separator(words);
        let end = split.unwrap_or(words.len());
        let compiler = words[..end]
            .iter()
            .map(|word| {
                word.to_str()
                    .map(str::to_owned)
                    .ok_or("compiler options and source paths must be valid UTF-8")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut program = vec![executable];
        if let Some(split) = split {
            program.extend_from_slice(&words[split + 1..]);
        }
        Ok(Self {
            compiler,
            original,
            program,
            separator: split,
        })
    }

    pub(crate) fn enter(&mut self) -> Result<Scope, &'static str> {
        install(std::mem::take(&mut self.program))
    }

    pub(crate) fn legacy(self, mut forwarded: Vec<String>) -> Result<Vec<String>, &'static str> {
        let mut original = self.original.into_iter();
        let executable = original.next().unwrap_or_else(|| "oxid".into());
        if let Some(separator) = self.separator {
            // A run command without a typed project may still belong to the
            // legacy runtime. Keep the already classified compiler prefix and
            // restore only its separator and original literal operands.
            forwarded.push("--".into());
            for word in original.skip(separator + 1) {
                forwarded.push(
                    word.into_string()
                        .map_err(|_| "legacy runtime arguments must be valid UTF-8")?,
                );
            }
        }
        forwarded.insert(
            0,
            executable
                .into_string()
                .map_err(|_| "legacy executable path must be valid UTF-8")?,
        );
        Ok(forwarded)
    }
}

fn takes_value(option: &str) -> bool {
    matches!(
        option,
        "--edition"
            | "--entry-mode"
            | "--message-format"
            | "--profile"
            | "--package"
            | "-p"
            | "--backend"
            | "--target"
            | "--output"
            | "--experimental-hir-import"
            | "--experimental-hir-observation"
            | "--experimental-hir-output"
            | "--experimental-hir-producer"
            | "--experimental-lexical-provider"
    )
}

/// Find a program separator without reading a manifest or interpreting source.
/// An explicit typed-file route may first use `--` to protect its source path;
/// the following separator then starts the program's literal arguments.
fn program_separator(words: &[OsString]) -> Option<usize> {
    let mut command = None;
    let mut source = false;
    let mut typed = false;
    let mut index = 0;
    while index < words.len() {
        let word = words[index].to_str()?;
        if word == "--" {
            if command != Some("run") {
                return None;
            }
            if !typed || source {
                return Some(index);
            }
            // Keep the protected source itself in the compiler's operands,
            // including a source file literally named `--`.
            return words
                .get(index + 2..)?
                .iter()
                .position(|word| word == OsStr::new("--"))
                .map(|position| index + 2 + position);
        }
        if let Some((option, value)) = word.split_once('=') {
            if option == "--edition" && value == "typed-preview" {
                typed = true;
            }
        } else if takes_value(word) {
            let value = words.get(index + 1)?.to_str()?;
            if word == "--edition" && value == "typed-preview" {
                typed = true;
            }
            index += 2;
            continue;
        }
        if !word.starts_with('-') {
            if command.is_none() {
                command = Some(word);
                if command != Some("run") {
                    return None;
                }
            } else {
                source = true;
            }
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(words: &[&str]) -> Invocation {
        Invocation::parse(words.iter().map(OsString::from).collect()).unwrap()
    }

    #[test]
    fn project_separator_preserves_empty_and_option_shaped_program_values() {
        let mut invocation = invocation(&["oxid", "run", "--", "", "--edition=legacy"]);
        assert_eq!(invocation.compiler, ["run"]);
        let _scope = invocation.enter().unwrap();
        assert_eq!(count(), 3);
        with_argument(0, |bytes| assert_eq!(bytes, Some(b"oxid".as_slice())));
        with_argument(1, |bytes| assert_eq!(bytes, Some(b"".as_slice())));
        with_argument(2, |bytes| {
            assert_eq!(bytes, Some(b"--edition=legacy".as_slice()))
        });
        with_argument(-1, |bytes| assert_eq!(bytes, None));
        with_argument(3, |bytes| assert_eq!(bytes, None));
    }

    #[test]
    fn explicit_source_and_protected_source_keep_their_compiler_operands() {
        for (words, compiler) in [
            (
                vec![
                    "oxid",
                    "--edition=typed-preview",
                    "run",
                    "main.ox",
                    "--",
                    "x",
                ],
                vec!["--edition=typed-preview", "run", "main.ox"],
            ),
            (
                vec![
                    "oxid",
                    "run",
                    "--edition",
                    "typed-preview",
                    "--",
                    "--main.ox",
                    "--",
                    "x",
                ],
                vec!["run", "--edition", "typed-preview", "--", "--main.ox"],
            ),
            (
                vec![
                    "oxid",
                    "--edition=typed-preview",
                    "run",
                    "--",
                    "--",
                    "--",
                    "x",
                ],
                vec!["--edition=typed-preview", "run", "--", "--"],
            ),
        ] {
            let mut invocation = invocation(&words);
            assert_eq!(invocation.compiler, compiler);
            let _scope = invocation.enter().unwrap();
            assert_eq!(count(), 2);
            with_argument(1, |bytes| assert_eq!(bytes, Some(b"x".as_slice())));
        }
    }

    #[test]
    fn no_separator_and_non_run_routes_retain_all_original_words() {
        for words in [
            vec!["oxid", "run", "--edition=typed-preview", "--", "main.ox"],
            vec!["oxid", "check", "--", "main.ox"],
            vec!["oxid", "script", "build", "--", "run"],
            vec!["oxid", "--", "run", "--", "x"],
        ] {
            let invocation = invocation(&words);
            assert_eq!(invocation.compiler, words[1..]);
        }
    }

    #[test]
    fn legacy_fallback_restores_separator_and_words_exactly() {
        let invocation = invocation(&["oxid", "run", "file.ox", "--", "value"]);
        let forwarded = invocation.compiler.clone();
        assert_eq!(
            invocation.legacy(forwarded).unwrap(),
            ["oxid", "run", "file.ox", "--", "value"]
        );
    }

    #[test]
    fn legacy_fallback_keeps_the_frontend_classified_prefix() {
        let invocation = invocation(&["oxid", "--edition=legacy", "run", "--", "value"]);
        assert_eq!(
            invocation.legacy(vec!["run".into()]).unwrap(),
            ["oxid", "run", "--", "value"]
        );
    }

    #[test]
    fn nested_context_restores_the_previous_argument_owner() {
        assert_eq!(count(), 0);
        {
            let _outer = install(vec!["outer".into()]).unwrap();
            {
                let _inner = install(vec!["inner".into(), "value".into()]).unwrap();
                assert_eq!(count(), 2);
            }
            assert_eq!(count(), 1);
            with_argument(0, |bytes| assert_eq!(bytes, Some(b"outer".as_slice())));
        }
        assert_eq!(count(), 0);
    }

    #[test]
    fn copy_prepays_scan_copy_and_return_before_touching_the_destination() {
        let _scope = install(vec!["oxid".into(), "abc".into(), "".into()]).unwrap();
        for fuel in 0..14 {
            let mut destination = *b"xxxxx";
            let mut remaining = fuel;
            assert_eq!(
                copy_argument(1, &mut destination, 7, &mut remaining),
                Err(())
            );
            assert_eq!(destination, *b"xxxxx");
            assert_eq!(remaining, fuel);
        }
        let mut destination = *b"xxxxx";
        let mut fuel = 14;
        assert_eq!(
            copy_argument(1, &mut destination, 7, &mut fuel),
            Ok((0, Some(3)))
        );
        assert_eq!((destination, fuel), (*b"abcxx", 0));
        let mut short = *b"xx";
        let mut fuel = 11;
        assert_eq!(copy_argument(1, &mut short, 7, &mut fuel), Ok((1, Some(3))));
        assert_eq!((short, fuel), (*b"xx", 0));
        assert_eq!(copy_argument(2, &mut [], 7, &mut 8), Ok((0, Some(0))));
        assert_eq!(copy_argument(-1, &mut [], 7, &mut 7), Ok((2, None)));
        assert_eq!(copy_argument(3, &mut [], 7, &mut 7), Ok((2, None)));
    }

    #[cfg(unix)]
    #[test]
    fn only_program_operands_accept_non_utf8_os_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(vec![255, b'x']);
        let mut invocation =
            Invocation::parse(vec!["oxid".into(), "run".into(), "--".into(), raw.clone()]).unwrap();
        let _scope = invocation.enter().unwrap();
        with_argument(1, |bytes| assert_eq!(bytes, Some([255, b'x'].as_slice())));
        assert!(Invocation::parse(vec!["oxid".into(), "check".into(), raw]).is_err());
    }
}
