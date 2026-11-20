#![allow(clippy::print_stdout, clippy::print_stderr)]

use crate::error::ArranqueError;
use std::io::{self, IsTerminal, Write};

pub struct Reporter {
    color_stdout: bool,
    color_stderr: bool,
}

impl Reporter {
    pub fn new(no_color: bool) -> Self {
        let env_no_color = no_color
            || std::env::var("NO_COLOR")
                .map(|v| !v.is_empty())
                .unwrap_or(false);
        let color_stdout = !env_no_color && io::stdout().is_terminal();
        let color_stderr = !env_no_color && io::stderr().is_terminal();
        Self {
            color_stdout,
            color_stderr,
        }
    }

    pub fn with_colors(color_stdout: bool, color_stderr: bool) -> Self {
        Self {
            color_stdout,
            color_stderr,
        }
    }

    fn paint_stdout(&self, code: &str, text: &str) -> String {
        if self.color_stdout {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn paint_stderr(&self, code: &str, text: &str) -> String {
        if self.color_stderr {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn title(&self, text: &str) {
        println!("\n{}\n", self.paint_stdout("1", text));
    }

    pub fn ok(&self, label: &str, detail: &str) {
        let tag = self.paint_stdout("32", "[ OK ]");
        let detail_str = if !detail.is_empty() {
            self.paint_stdout("90", &format!("  {detail}"))
        } else {
            String::new()
        };
        println!("  {tag} {label}{detail_str}");
    }

    pub fn info(&self, label: &str, detail: &str) {
        let tag = self.paint_stdout("36", "[INFO]");
        let detail_str = if !detail.is_empty() {
            self.paint_stdout("90", &format!("  {detail}"))
        } else {
            String::new()
        };
        println!("  {tag} {label}{detail_str}");
    }

    pub fn warn(&self, label: &str, lines: &[&str]) {
        let tag = self.paint_stderr("33", "[WARN]");
        eprintln!("  {tag} {label}");
        for line in lines {
            eprintln!("         {line}");
        }
    }

    pub fn fail(&self, label: &str, lines: &[&str]) {
        let tag = self.paint_stderr("31", "[FAIL]");
        let painted_label = self.paint_stderr("1", label);
        eprintln!("  {tag} {painted_label}");
        for line in lines {
            eprintln!("         {line}");
        }
    }

    pub fn stop(&self, problems: usize) -> ArranqueError {
        let msg =
            format!("The bot was not started: {problems} problem(s) above must be fixed first.");
        let painted = self.paint_stderr("31;1", &msg);
        eprintln!("\n{painted}");
        eprintln!("Fix them and start the bot again. Every step is also explained in README.md.\n");
        let _ = io::stderr().flush();
        ArranqueError::new(problems)
    }
}
