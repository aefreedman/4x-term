use std::{ffi::OsString, path::PathBuf};

pub const HELP: &str = "4x-term\n\nUSAGE:\n    4x-term [OPTIONS]\n\nOPTIONS:\n    -T, --playtest-trace [PATH]  Write a local playtest trace. Without PATH, uses\n                                 playtest-logs/playtest-<unix-ms>-p<pid>.ronl\n        --playtest-trace=PATH    Write to PATH without overwriting it\n    -h, --help                   Print help\n";

#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Help,
    Run { trace: TraceRequest },
}

#[derive(Debug, Eq, PartialEq)]
pub enum TraceRequest {
    Disabled,
    Default,
    Explicit(PathBuf),
}

pub fn parse<I>(arguments: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let arguments: Vec<OsString> = arguments.into_iter().collect();
    let mut trace = None;
    let mut help = false;
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index].to_string_lossy();
        match argument.as_ref() {
            "-h" | "--help" => {
                if help {
                    return Err("the help option was provided more than once".into());
                }
                help = true;
                index += 1;
            }
            "-T" | "--playtest-trace" => {
                if trace.is_some() {
                    return Err("the playtest trace option was provided more than once".into());
                }
                let value = arguments
                    .get(index + 1)
                    .filter(|value| !value.to_string_lossy().starts_with('-'))
                    .map(PathBuf::from);
                let consumed_value = value.is_some();
                trace = Some(value.map_or(TraceRequest::Default, TraceRequest::Explicit));
                index += if consumed_value { 2 } else { 1 };
            }
            _ if argument.starts_with("--playtest-trace=") => {
                if trace.is_some() {
                    return Err("the playtest trace option was provided more than once".into());
                }
                let (_, value) = argument
                    .split_once('=')
                    .expect("matched prefix contains equals");
                if value.is_empty() {
                    return Err("--playtest-trace= requires a non-empty path".into());
                }
                trace = Some(TraceRequest::Explicit(PathBuf::from(value)));
                index += 1;
            }
            _ if argument.starts_with('-') => {
                return Err(format!("unknown option: {argument}"));
            }
            _ => {
                return Err(format!("unexpected positional argument: {argument}"));
            }
        }
    }

    if help {
        if trace.is_some() {
            return Err("--help cannot be combined with --playtest-trace".into());
        }
        Ok(Command::Help)
    } else {
        Ok(Command::Run {
            trace: trace.unwrap_or(TraceRequest::Disabled),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_disable_tracing() {
        assert_eq!(
            parse(args(&[])),
            Ok(Command::Run {
                trace: TraceRequest::Disabled
            })
        );
    }

    #[test]
    fn bare_short_and_long_options_choose_the_default() {
        assert_eq!(
            parse(args(&["-T"])),
            Ok(Command::Run {
                trace: TraceRequest::Default
            })
        );
        assert_eq!(
            parse(args(&["--playtest-trace"])),
            Ok(Command::Run {
                trace: TraceRequest::Default
            })
        );
    }

    #[test]
    fn all_explicit_path_forms_agree() {
        let expected = Ok(Command::Run {
            trace: TraceRequest::Explicit(PathBuf::from("somewhere/session.ronl")),
        });
        assert_eq!(parse(args(&["-T", "somewhere/session.ronl"])), expected);
        assert_eq!(
            parse(args(&["--playtest-trace", "somewhere/session.ronl"])),
            Ok(Command::Run {
                trace: TraceRequest::Explicit(PathBuf::from("somewhere/session.ronl"))
            })
        );
        assert_eq!(
            parse(args(&["--playtest-trace=somewhere/session.ronl"])),
            Ok(Command::Run {
                trace: TraceRequest::Explicit(PathBuf::from("somewhere/session.ronl"))
            })
        );
    }

    #[test]
    fn duplicate_unknown_and_positional_arguments_are_rejected() {
        assert!(parse(args(&["-T", "--playtest-trace"])).is_err());
        assert!(parse(args(&["--unknown"])).is_err());
        assert!(parse(args(&["session.ronl"])).is_err());
        assert!(parse(args(&["--playtest-trace="])).is_err());
    }

    #[test]
    fn help_documents_default_and_explicit_forms() {
        assert!(HELP.contains("-T, --playtest-trace [PATH]"));
        assert!(HELP.contains("--playtest-trace=PATH"));
        assert!(HELP.contains("playtest-logs/playtest-<unix-ms>-p<pid>.ronl"));
    }
}
