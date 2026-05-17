use std::panic::Location;

pub trait ResultExt<E> {
    type Ok;

    fn log_err(self) -> Option<Self::Ok>;
    fn warn_on_err(self) -> Option<Self::Ok>;
}

impl<T, E> ResultExt<E> for Result<T, E>
where
    E: std::fmt::Display,
{
    type Ok = T;

    #[track_caller]
    fn log_err(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                log_error_with_caller(*Location::caller(), error, log::Level::Error);
                None
            }
        }
    }

    #[track_caller]
    fn warn_on_err(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                log_error_with_caller(*Location::caller(), error, log::Level::Warn);
                None
            }
        }
    }
}

fn log_error_with_caller<E>(caller: core::panic::Location<'_>, error: E, level: log::Level)
where
    E: std::fmt::Display,
{
    #[cfg(not(windows))]
    let file = caller.file();
    #[cfg(windows)]
    let file = caller.file().replace('\\', "/");
    let file = file.split_once("crates/");
    let target = file.as_ref().and_then(|(_, s)| s.split_once("/src/"));

    let module_path = target.map(|(krate, module)| {
        if module.starts_with(krate) {
            module.trim_end_matches(".rs").replace('/', "::")
        } else {
            krate.to_owned() + "::" + &module.trim_end_matches(".rs").replace('/', "::")
        }
    });
    let file = file.map(|(_, file)| format!("crates/{file}"));
    log::logger().log(
        &log::Record::builder()
            .target(module_path.as_deref().unwrap_or(""))
            .module_path(file.as_deref())
            .args(format_args!("{:#}", error))
            .file(Some(caller.file()))
            .line(Some(caller.line()))
            .level(level)
            .build(),
    );
}
