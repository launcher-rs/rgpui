use std::borrow::Cow;

use super::shell::get_system_shell;
use super::shell::{Shell, ShellKind};

/// ShellBuilder 用于将用户请求的任务转换为
/// 可由 shell 执行的程序。
pub struct ShellBuilder {
    /// 要运行的 shell
    program: String,
    args: Vec<String>,
    interactive: bool,
    /// 是否将生成命令的 stdin 重定向到 /dev/null。
    redirect_stdin: bool,
    kind: ShellKind,
}

impl ShellBuilder {
    /// 创建一个新的 ShellBuilder。
    pub fn new(shell: &Shell, is_windows: bool) -> Self {
        let (program, args) = match shell {
            Shell::System => (get_system_shell(), Vec::new()),
            Shell::Program(shell) => (shell.clone(), Vec::new()),
            Shell::WithArguments { program, args, .. } => (program.clone(), args.clone()),
        };

        let kind = ShellKind::new(&program, is_windows);
        Self {
            program,
            args,
            interactive: true,
            kind,
            redirect_stdin: false,
        }
    }
    pub fn non_interactive(mut self) -> Self {
        self.interactive = false;
        self
    }

    /// 返回要在终端标签中显示的标签。
    pub fn command_label(&self, command_to_use_in_label: &str) -> String {
        if command_to_use_in_label.trim().is_empty() {
            self.program.clone()
        } else {
            match self.kind {
                ShellKind::PowerShell | ShellKind::Pwsh => {
                    format!("{} -C '{}'", self.program, command_to_use_in_label)
                }
                ShellKind::Cmd => {
                    format!("{} /C \"{}\"", self.program, command_to_use_in_label)
                }
                ShellKind::Posix
                | ShellKind::Nushell
                | ShellKind::Fish
                | ShellKind::Csh
                | ShellKind::Tcsh
                | ShellKind::Rc
                | ShellKind::Xonsh
                | ShellKind::Elvish => {
                    let interactivity = self.interactive.then_some("-i ").unwrap_or_default();
                    format!(
                        "{PROGRAM} {interactivity}-c '{command_to_use_in_label}'",
                        PROGRAM = self.program
                    )
                }
            }
        }
    }

    pub fn redirect_stdin_to_dev_null(mut self) -> Self {
        self.redirect_stdin = true;
        self
    }

    /// 返回运行此任务的程序和参数。
    pub fn build(
        mut self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> (String, Vec<String>) {
        if let Some(task_command) = task_command {
            let task_command = if !task_args.is_empty() {
                match self.kind.try_quote_prefix_aware(&task_command) {
                    Some(task_command) => task_command.into_owned(),
                    None => task_command,
                }
            } else {
                task_command
            };
            let mut combined_command = task_args.iter().fold(task_command, |mut command, arg| {
                command.push(' ');
                let shell_variable = self.kind.to_shell_variable(arg);
                command.push_str(&match self.kind.try_quote(&shell_variable) {
                    Some(shell_variable) => shell_variable,
                    None => Cow::Owned(shell_variable),
                });
                command
            });
            if self.redirect_stdin {
                match self.kind {
                    ShellKind::Fish => {
                        combined_command.insert_str(0, "begin; ");
                        combined_command.push_str("; end </dev/null");
                    }
                    ShellKind::Posix
                    | ShellKind::Nushell
                    | ShellKind::Csh
                    | ShellKind::Tcsh
                    | ShellKind::Rc
                    | ShellKind::Xonsh
                    | ShellKind::Elvish => {
                        combined_command.insert(0, '(');
                        combined_command.push_str("\n) </dev/null");
                    }
                    ShellKind::PowerShell | ShellKind::Pwsh => {
                        combined_command.insert_str(0, "$null | & {");
                        combined_command.push_str("}");
                    }
                    ShellKind::Cmd => {
                        combined_command.push_str("< NUL");
                    }
                }
            }

            self.args
                .extend(self.kind.args_for_shell(self.interactive, combined_command));
        }

        (self.program, self.args)
    }

    // 此方法不应存在，但我们的任务基础设施目前存在问题
    #[doc(hidden)]
    pub fn build_no_quote(
        mut self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> (String, Vec<String>) {
        if let Some(task_command) = task_command {
            let mut combined_command = task_args.iter().fold(task_command, |mut command, arg| {
                command.push(' ');
                command.push_str(&self.kind.to_shell_variable(arg));
                command
            });
            if self.redirect_stdin {
                match self.kind {
                    ShellKind::Fish => {
                        combined_command.insert_str(0, "begin; ");
                        combined_command.push_str("; end </dev/null");
                    }
                    ShellKind::Posix
                    | ShellKind::Nushell
                    | ShellKind::Csh
                    | ShellKind::Tcsh
                    | ShellKind::Rc
                    | ShellKind::Xonsh
                    | ShellKind::Elvish => {
                        combined_command.insert(0, '(');
                        combined_command.push_str("\n) </dev/null");
                    }
                    ShellKind::PowerShell | ShellKind::Pwsh => {
                        combined_command.insert_str(0, "$null | & {");
                        combined_command.push_str("}");
                    }
                    ShellKind::Cmd => {
                        combined_command.push_str("< NUL");
                    }
                }
            }

            self.args
                .extend(self.kind.args_for_shell(self.interactive, combined_command));
        }

        (self.program, self.args)
    }

    /// 使用给定任务命令和参数构建 `smol::process::Command`。
    ///
    /// 优先使用此方法而非手动使用 `Self::build` 的输出构造命令，
    /// 因为此方法正确处理 Windows 上 `cmd` 的怪异行为。
    pub fn build_smol_command(
        self,
        task_command: Option<String>,
        task_args: &[String],
    ) -> smol::process::Command {
        smol::process::Command::from(self.build_std_command(task_command, task_args))
    }

    /// 使用给定任务命令和参数构建 `std::process::Command`。
    ///
    /// 优先使用此方法而非手动使用 `Self::build` 的输出构造命令，
    /// 因为此方法正确处理 Windows 上 `cmd` 的怪异行为。
    pub fn build_std_command(
        self,
        mut task_command: Option<String>,
        task_args: &[String],
    ) -> std::process::Command {
        #[cfg(windows)]
        let kind = self.kind;
        if task_args.is_empty() {
            task_command = task_command
                .as_ref()
                .map(|cmd| self.kind.try_quote_prefix_aware(&cmd).map(Cow::into_owned))
                .unwrap_or(task_command);
        }
        let (program, args) = self.build(task_command, task_args);

        let mut child = super::command::new_std_command(program);

        #[cfg(windows)]
        if kind == ShellKind::Cmd {
            use std::os::windows::process::CommandExt;

            for arg in args {
                child.raw_arg(arg);
            }
        } else {
            child.args(args);
        }

        #[cfg(not(windows))]
        child.args(args);

        child
    }

    pub fn kind(&self) -> ShellKind {
        self.kind
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_nu_shell_variable_substitution() {
        let shell = Shell::Program("nu".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder.build(
            Some("echo".into()),
            &[
                "${hello}".to_string(),
                "$world".to_string(),
                "nothing".to_string(),
                "--$something".to_string(),
                "$".to_string(),
                "${test".to_string(),
            ],
        );

        assert_eq!(program, "nu");
        assert_eq!(
            args,
            vec![
                "-i",
                "-c",
                "echo '$env.hello' '$env.world' nothing '--($env.something)' '$' '${test'"
            ]
        );
    }

    #[test]
    fn redirect_stdin_to_dev_null_precedence() {
        let shell = Shell::Program("nu".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder
            .redirect_stdin_to_dev_null()
            .build(Some("echo".into()), &["nothing".to_string()]);

        assert_eq!(program, "nu");
        assert_eq!(args, vec!["-i", "-c", "(echo nothing\n) </dev/null"]);
    }

    #[test]
    fn redirect_stdin_to_dev_null_fish() {
        let shell = Shell::Program("fish".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder
            .redirect_stdin_to_dev_null()
            .build(Some("echo".into()), &["test".to_string()]);

        assert_eq!(program, "fish");
        assert_eq!(args, vec!["-i", "-c", "begin; echo test; end </dev/null"]);
    }

    #[test]
    fn redirect_stdin_to_dev_null_preserves_heredoc() {
        let shell = Shell::Program("sh".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let command = "cat <<EOF\nhello\nEOF";
        let (program, args) = shell_builder
            .redirect_stdin_to_dev_null()
            .build(Some(command.into()), &[]);

        assert_eq!(program, "sh");
        assert_eq!(
            args,
            vec!["-i", "-c", "(cat <<EOF\nhello\nEOF\n) </dev/null"]
        );
    }

    #[test]
    fn does_not_quote_sole_command_only() {
        let shell = Shell::Program("fish".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder.build(Some("echo".into()), &[]);

        assert_eq!(program, "fish");
        assert_eq!(args, vec!["-i", "-c", "echo"]);

        let shell = Shell::Program("fish".to_owned());
        let shell_builder = ShellBuilder::new(&shell, false);

        let (program, args) = shell_builder.build(Some("echo oo".into()), &[]);

        assert_eq!(program, "fish");
        assert_eq!(args, vec!["-i", "-c", "echo oo"]);
    }
}
