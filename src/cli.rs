use std::path::PathBuf;

pub enum Command {
    Run { data: Option<PathBuf> },
    Help,
    Version,
    Where { data: Option<PathBuf> },
    Install,
    Uninstall { keep_data: bool },
}

pub fn parse<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut data: Option<PathBuf> = None;
    let mut uninstall = false;
    let mut keep_data = false;
    let mut where_ = false;
    let mut install = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--where" => where_ = true,
            "--install" => install = true,
            "--uninstall" => uninstall = true,
            "-k" | "--keep-data" => keep_data = true,
            "--data" => {
                let value = args.next().ok_or_else(|| "--data 需要一个路径".to_string())?;
                data = Some(PathBuf::from(value));
            }
            other if other.starts_with("--data=") => {
                data = Some(PathBuf::from(&other["--data=".len()..]));
            }
            other if other.starts_with('-') => {
                return Err(format!("未知参数 {other}"));
            }
            other => {
                return Err(format!("多余的参数 {other}"));
            }
        }
    }

    Ok(if uninstall {
        Command::Uninstall { keep_data }
    } else if install {
        Command::Install
    } else if where_ {
        Command::Where { data }
    } else {
        Command::Run { data }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn defaults_to_run() {
        assert!(matches!(parse(args(&[])).unwrap(), Command::Run { data: None }));
    }

    #[test]
    fn parses_data_flag() {
        match parse(args(&["--data", "x.txt"])).unwrap() {
            Command::Run { data } => assert_eq!(data.unwrap(), PathBuf::from("x.txt")),
            _ => panic!("expected run"),
        }
        match parse(args(&["--data=y.txt"])).unwrap() {
            Command::Run { data } => assert_eq!(data.unwrap(), PathBuf::from("y.txt")),
            _ => panic!("expected run"),
        }
    }

    #[test]
    fn rejects_unknown_flags() {
        assert!(parse(args(&["--nope"])).is_err());
    }

    #[test]
    fn uninstall_keeps_data_flag() {
        assert!(matches!(
            parse(args(&["--uninstall", "-k"])).unwrap(),
            Command::Uninstall { keep_data: true }
        ));
    }
}
