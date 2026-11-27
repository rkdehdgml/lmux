//! 세션에서 실행할 에이전트 명령 한 줄. 셸이 뜬 뒤 그 줄을 입력한다.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    #[default]
    None,
    Claude,
    Codex,
    Custom,
}

/// 인용 규칙이 같은 셸 묶음.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellKind {
    PowerShell,
    Cmd,
    Posix,
}

/// 줄바꿈·탭 같은 문자는 입력하면 키로 동작하므로 공백으로 바꾼다.
fn flatten(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{2028}' | '\u{2029}' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect()
}

/// 첫 지시를 셸에 한 인수로 넘기는 형태. 넘길 것이 없으면 None.
pub fn argument(kind: ShellKind, prompt: &str) -> Option<String> {
    let mut text = flatten(prompt).trim().to_string();
    // Windows의 네이티브 명령 인수 전달은 큰따옴표와 끝의 역슬래시를 깨뜨린다.
    if kind != ShellKind::Posix {
        text = text.replace('"', "'").replace('%', "％");
        text = text.trim_end_matches('\\').to_string();
    }
    if text.is_empty() {
        return None;
    }
    // 옵션으로 해석되지 않게 한다.
    if text.starts_with('-') {
        text.insert(0, ' ');
    }
    Some(match kind {
        ShellKind::PowerShell => {
            // 공백이 없으면 인용 없이 넘어가 .cmd 셔임에서 &가 명령을 나눈다.
            if !text.contains(' ') {
                text.push(' ');
            }
            // PowerShell은 굽은 작은따옴표도 작은따옴표로 본다.
            for quote in ['\'', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'] {
                text = text.replace(quote, &format!("{quote}{quote}"));
            }
            format!("'{text}'")
        }
        ShellKind::Cmd => format!("\"{text}\""),
        ShellKind::Posix => format!("'{}'", text.replace('\'', r"'\''")),
    })
}

/// 입력할 명령 줄. 에이전트가 없으면 None.
pub fn command_line(kind: ShellKind, agent: Agent, custom: &str, prompt: &str) -> Option<String> {
    let program = match agent {
        Agent::None => return None,
        Agent::Claude => "claude",
        Agent::Codex => "codex",
        // 사용자가 직접 쓴 명령은 그대로 입력한다.
        Agent::Custom => {
            return Some(flatten(custom).trim().to_string()).filter(|c| !c.is_empty());
        }
    };
    let line = match argument(kind, prompt) {
        Some(argument) => format!("{program} {argument}"),
        None => program.to_string(),
    };
    Some(match kind {
        // cmd는 현재 폴더를 먼저 찾아, 프로젝트에 심은 `claude.cmd`가 대신 실행될 수 있다.
        ShellKind::Cmd => format!("set \"NoDefaultCurrentDirectoryInExePath=1\" & {line}"),
        _ => line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ShellKind::{Cmd, Posix, PowerShell};

    fn claude(kind: ShellKind, prompt: &str) -> String {
        command_line(kind, Agent::Claude, "", prompt).unwrap()
    }

    #[test]
    fn runs_the_agent_with_no_argument_when_there_is_no_prompt() {
        assert_eq!(command_line(PowerShell, Agent::None, "", "x"), None);
        assert_eq!(claude(PowerShell, ""), "claude");
        assert_eq!(claude(Posix, " \r\n "), "claude");
        assert_eq!(
            command_line(Cmd, Agent::Codex, "", "").as_deref(),
            Some(r#"set "NoDefaultCurrentDirectoryInExePath=1" & codex"#)
        );
    }

    #[test]
    fn quotes_the_prompt_for_each_shell() {
        assert_eq!(
            claude(PowerShell, "it's $(x) done"),
            "claude 'it''s $(x) done'"
        );
        assert_eq!(
            claude(Posix, "it's $(x) done"),
            r"claude 'it'\''s $(x) done'"
        );
        assert_eq!(
            claude(Cmd, "a & b"),
            r#"set "NoDefaultCurrentDirectoryInExePath=1" & claude "a & b""#
        );
    }

    #[test]
    fn doubles_the_curly_quotes_powershell_treats_as_quotes() {
        assert_eq!(
            claude(PowerShell, "a \u{2019}; calc \u{2018}\u{201a}\u{201b}"),
            "claude 'a \u{2019}\u{2019}; calc \u{2018}\u{2018}\u{201a}\u{201a}\u{201b}\u{201b}'"
        );
    }

    #[test]
    fn removes_characters_that_would_act_as_keys_or_break_quoting() {
        assert_eq!(
            claude(Posix, "a\r\nb\tc\u{3}d\u{2028}e"),
            "claude 'a  b c d e'"
        );
        assert_eq!(
            claude(PowerShell, "say \"hi\" now"),
            "claude 'say ''hi'' now'"
        );
        assert_eq!(argument(Cmd, "say \"hi\" now").unwrap(), "\"say 'hi' now\"");
        assert_eq!(argument(Cmd, "%PATH% 50%").unwrap(), "\"％PATH％ 50％\"");
        assert_eq!(claude(PowerShell, "%PATH% x"), "claude '％PATH％ x'");
        assert_eq!(claude(Posix, "50% x"), "claude '50% x'");
    }

    #[test]
    fn keeps_a_prompt_from_becoming_an_option_or_splitting_in_a_cmd_shim() {
        // 공백이 없으면 PowerShell이 인용 없이 넘겨 .cmd 셔임에서 &가 명령을 나눈다.
        assert_eq!(claude(PowerShell, "a&calc"), "claude 'a&calc '");
        assert_eq!(claude(Posix, "--help me"), "claude ' --help me'");
        assert_eq!(claude(PowerShell, r"in C:\dir \\"), r"claude 'in C:\dir '");
    }

    #[test]
    fn types_a_custom_command_as_written() {
        assert_eq!(
            command_line(Cmd, Agent::Custom, "  aider --model \"x\"\n", "ignored").as_deref(),
            Some("aider --model \"x\"")
        );
        assert_eq!(command_line(Cmd, Agent::Custom, "  ", ""), None);
    }
}
