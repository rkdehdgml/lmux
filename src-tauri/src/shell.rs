//! 셸 감지와 실행 명령. 실행 파일은 PATH를 뒤지지 않고 정해진 설치 위치의 절대 경로만 쓴다.

use crate::agent::ShellKind;
use portable_pty::CommandBuilder;
use serde::Serialize;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const ATTRIBUTE_DIRECTORY: u32 = 0x10;
const ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const WSL_TIMEOUT: Duration = Duration::from_secs(5);
const WSL_PREFIX: &str = "wsl:";

/// PowerShell을 UTF-8로 시작시키는 초기화 명령.
const PS_UTF8: &str = "[Console]::InputEncoding = [Console]::OutputEncoding = \
    [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding";

#[derive(Serialize, Clone, Debug)]
pub struct ShellInfo {
    pub id: String,
    pub name: String,
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

fn system32(file: &str) -> PathBuf {
    env_dir("SystemRoot")
        .unwrap_or_else(|| r"C:\Windows".into())
        .join("System32")
        .join(file)
}

fn powershell7_path() -> Option<PathBuf> {
    let msi = env_dir("ProgramFiles")
        .map(|d| d.join(r"PowerShell\7\pwsh.exe"))
        .filter(|p| p.is_file());
    // 스토어 앱 실행 별칭은 재분석 지점이라 일반 파일 검사에 실패한다. 그 형태만 받는다.
    let store = || {
        env_dir("LOCALAPPDATA")
            .map(|d| d.join(r"Microsoft\WindowsApps\pwsh.exe"))
            .filter(|p| {
                p.symlink_metadata().is_ok_and(|m| {
                    m.file_attributes() & (ATTRIBUTE_DIRECTORY | ATTRIBUTE_REPARSE_POINT)
                        == ATTRIBUTE_REPARSE_POINT
                })
            })
    };
    msi.or_else(store)
}

/// PowerShell 7(MSI 설치 또는 스토어 설치)이 있으면 그것을, 없으면 Windows PowerShell을 쓴다.
pub fn powershell_path() -> PathBuf {
    powershell7_path().unwrap_or_else(|| system32(r"WindowsPowerShell\v1.0\powershell.exe"))
}

fn git_bash_path() -> Option<PathBuf> {
    [
        ("ProgramFiles", r"Git\bin\bash.exe"),
        ("ProgramFiles(x86)", r"Git\bin\bash.exe"),
        ("LOCALAPPDATA", r"Programs\Git\bin\bash.exe"),
    ]
    .iter()
    .filter_map(|(base, rest)| env_dir(base).map(|d| d.join(rest)))
    .find(|p| p.is_file())
}

/// `wsl.exe -l -q`의 UTF-16LE 출력에서 배포판 이름을 읽는다.
pub fn parse_wsl_list(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|line| line.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}'))
        // Docker Desktop의 내부 배포판은 작업용 셸이 아니다.
        .filter(|name| !name.is_empty() && !name.starts_with("docker-desktop"))
        .map(String::from)
        .collect()
}

/// WSL이 없거나 응답하지 않으면 빈 목록.
fn wsl_distros() -> Vec<String> {
    let mut command = Command::new(system32("wsl.exe"));
    command
        .args(["-l", "-q"])
        .env_remove("WSL_UTF8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    let Ok(mut child) = command.spawn() else {
        return Vec::new();
    };
    // 출력은 짧아 파이프가 차지 않으므로 끝난 뒤에 읽는다.
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(None) if start.elapsed() < WSL_TIMEOUT => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Vec::new();
            }
        }
    }
    let mut output = Vec::new();
    if let Some(mut pipe) = child.stdout.take()
        && pipe.read_to_end(&mut output).is_err()
    {
        return Vec::new();
    }
    parse_wsl_list(&output)
}

/// 이 PC에서 고를 수 있는 셸. WSL 확인은 몇 초 걸릴 수 있어 뺄 수 있다.
pub fn list(include_wsl: bool) -> Vec<ShellInfo> {
    let info = |id: &str, name: &str| ShellInfo {
        id: id.into(),
        name: name.into(),
    };
    let ps = if powershell7_path().is_some() {
        "PowerShell 7"
    } else {
        "Windows PowerShell"
    };
    let mut shells = vec![info("powershell", ps)];
    let distros = if include_wsl {
        wsl_distros()
    } else {
        Vec::new()
    };
    for distro in distros {
        shells.push(info(
            &format!("{WSL_PREFIX}{distro}"),
            &format!("WSL · {distro}"),
        ));
    }
    if git_bash_path().is_some() {
        shells.push(info("gitbash", "Git Bash"));
    }
    shells.push(info("cmd", "cmd"));
    shells
}

/// 셸 id의 인용 규칙. 모르는 id는 오류.
pub fn kind(id: &str) -> Result<ShellKind, String> {
    match id {
        "powershell" => Ok(ShellKind::PowerShell),
        "cmd" => Ok(ShellKind::Cmd),
        "gitbash" => Ok(ShellKind::Posix),
        _ => id
            .strip_prefix(WSL_PREFIX)
            // 이름이 `-`로 시작하면 wsl.exe의 옵션이 된다.
            .filter(|distro| !distro.is_empty() && !distro.starts_with('-'))
            .map(|_| ShellKind::Posix)
            .ok_or_else(|| format!("알 수 없는 셸이에요: {id}")),
    }
}

/// `\\wsl$\<배포판>\…` 경로는 배포판 안의 경로로 바꾼다. 그 밖에는 그대로 둔다.
pub fn wsl_path(cwd: &str, distro: &str) -> Result<String, String> {
    let Some(share) = cwd.strip_prefix(r"\\") else {
        return Ok(cwd.into());
    };
    let mut parts = share.splitn(3, '\\');
    let host = parts.next().unwrap_or_default();
    if !host.eq_ignore_ascii_case("wsl$") && !host.eq_ignore_ascii_case("wsl.localhost") {
        return Ok(cwd.into());
    }
    let owner = parts.next().unwrap_or_default();
    if owner.to_lowercase() != distro.to_lowercase() {
        return Err(format!(
            "이 폴더는 WSL {owner} 안에 있어요. {distro} 셸에서는 열 수 없어요"
        ));
    }
    Ok(format!(
        "/{}",
        parts.next().unwrap_or_default().replace('\\', "/")
    ))
}

/// 에이전트 명령을 셸에 넘기는 환경 변수. 인수 인용을 거치지 않는다.
const RUN_ENV: &str = "LMUX_CMD";
/// PowerShell은 프로필을 읽은 뒤 `-Command`를 실행한다. 변수는 에이전트에 물려주지 않는다.
const PS_RUN: &str = "$lmux = $env:LMUX_CMD; Remove-Item Env:LMUX_CMD; Invoke-Expression $lmux";

/// 로그인 셸이 시작 파일을 읽은 뒤 명령을 실행하고, 끝나면 대화형 셸을 남긴다.
/// 에이전트가 물려받지 않게 변수는 먼저 지우고, 남는 셸은 로그인 시작 파일을 다시 읽지 않는다.
fn posix_run(next: &str) -> String {
    // 자동 내보내기(`set -a`)나 이미 내보낸 같은 이름이 있어도 복사본이 넘어가지 않게 한다.
    format!(
        "set +a; unset lmux; lmux=${RUN_ENV}; unset {RUN_ENV}; eval \"$lmux\"; unset lmux; exec {next} -i"
    )
}

/// WSL에서는 bash가 있으면 bash, 없으면 sh로 `$0`의 스크립트를 실행한다.
const WSL_LOGIN: &str = "S=$(command -v bash || command -v sh); exec \"$S\" -lic \"$0\"";

/// `cwd`에서 시작하는 셸 명령. `run`은 셸이 뜨면 실행할 명령 줄이다.
/// 시작 파일이 입력을 읽어도 빼앗기지 않게 시작 인수로 넘기고, 그럴 수 없는 cmd만
/// 돌려받은 줄을 셸이 뜬 뒤 입력한다.
pub fn command(
    id: &str,
    cwd: &str,
    run: Option<&str>,
) -> Result<(CommandBuilder, Option<String>), String> {
    kind(id)?;
    if id == "cmd" && cwd.starts_with(r"\\") {
        return Err("cmd는 네트워크 경로를 시작 폴더로 쓸 수 없어요".into());
    }
    // 없는 폴더를 넘기면 다른 폴더에서 셸이 뜬다.
    if !Path::new(cwd).is_dir() {
        return Err(format!("프로젝트 폴더가 없어요: {cwd}"));
    }
    let mut cmd;
    let mut typed = None;
    if let Some(distro) = id.strip_prefix(WSL_PREFIX) {
        cmd = CommandBuilder::new(system32("wsl.exe"));
        cmd.args(["-d", distro, "--cd", &wsl_path(cwd, distro)?]);
        if let Some(line) = run {
            // `--exec`는 기본 셸이 인수를 다시 해석하지 않게 한다.
            let script = posix_run("\"${SHELL:-sh}\"");
            cmd.args(["--exec", "sh", "-c", WSL_LOGIN, &script]);
            cmd.env(RUN_ENV, line);
            // 이 목록에 있는 변수만 WSL 안으로 전달된다.
            let shared = match cmd.get_env("WSLENV").and_then(|v| v.to_str()) {
                Some(list) if !list.is_empty() => format!("{list}:{RUN_ENV}/u"),
                _ => format!("{RUN_ENV}/u"),
            };
            cmd.env("WSLENV", shared);
        }
        // 시작 폴더는 --cd로 넘긴다. 네트워크 경로는 프로세스 폴더로 쓰지 않는다.
        if let Some(home) = env_dir("USERPROFILE") {
            cmd.cwd(home);
        }
    } else {
        match id {
            "cmd" => {
                cmd = CommandBuilder::new(system32("cmd.exe"));
                // 전체 경로로 불러야 프로젝트 폴더의 `chcp.cmd`가 실행되지 않는다.
                let utf8 = format!("{} 65001>nul", system32("chcp.com").display());
                // 에이전트를 띄울 때는 입력을 읽는 AutoRun이 시작 명령을 가져가지 않게 건너뛴다.
                if run.is_some() {
                    cmd.arg("/D");
                }
                // 지연 확장이 켜져 있으면 첫 지시의 `!`가 바뀐다.
                cmd.args(["/V:OFF", "/K", &utf8]);
                typed = run.map(String::from);
            }
            "gitbash" => {
                let bash = git_bash_path().ok_or("Git Bash를 찾지 못했어요")?;
                cmd = CommandBuilder::new(bash);
                cmd.args(["--login", "-i"]);
                if let Some(line) = run {
                    cmd.args(["-c", &posix_run("bash")]);
                    cmd.env(RUN_ENV, line);
                }
                // 로그인 셸이 홈 폴더로 옮기지 않게 한다.
                cmd.env("CHERE_INVOKING", "1");
                if std::env::var_os("LANG").is_none() {
                    cmd.env("LANG", "ko_KR.UTF-8");
                }
            }
            _ => {
                cmd = CommandBuilder::new(powershell_path());
                let init = match run {
                    Some(line) => {
                        cmd.env(RUN_ENV, line);
                        format!("{PS_UTF8}; {PS_RUN}")
                    }
                    None => PS_UTF8.into(),
                };
                cmd.args(["-NoLogo", "-NoExit", "-Command", &init]);
            }
        }
        cmd.cwd(cwd);
    }
    prepare_env(&mut cmd);
    Ok((cmd, typed))
}

fn prepare_env(cmd: &mut CommandBuilder) {
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // Lmux를 Claude Code 안에서 띄워도 패널의 claude가 중첩 실행으로 오인하지 않게 한다.
    for key in [
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_SSE_PORT",
    ] {
        cmd.env_remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn reads_distro_names_from_utf16_output() {
        let bytes =
            utf16("\u{feff}Ubuntu\r\ndocker-desktop\r\n\r\ndocker-desktop-data\r\n한글 배포판\r\n");
        assert_eq!(parse_wsl_list(&bytes), ["Ubuntu", "한글 배포판"]);
        assert!(parse_wsl_list(b"").is_empty());
    }

    #[test]
    fn converts_wsl_share_paths_of_the_same_distro() {
        for path in [
            r"\\wsl$\Ubuntu\home\a b",
            r"\\wsl.localhost\ubuntu\home\a b",
        ] {
            assert_eq!(wsl_path(path, "Ubuntu").unwrap(), "/home/a b");
        }
        assert_eq!(wsl_path(r"\\wsl$\Ubuntu", "Ubuntu").unwrap(), "/");
        assert_eq!(wsl_path(r"D:\work", "Ubuntu").unwrap(), r"D:\work");
        assert!(wsl_path(r"\\wsl$\Debian\home", "Ubuntu").is_err());
    }

    #[test]
    fn knows_the_quoting_rules_of_each_shell() {
        assert_eq!(kind("powershell").unwrap(), ShellKind::PowerShell);
        assert_eq!(kind("cmd").unwrap(), ShellKind::Cmd);
        assert_eq!(kind("gitbash").unwrap(), ShellKind::Posix);
        assert_eq!(kind("wsl:Ubuntu").unwrap(), ShellKind::Posix);
        for bad in ["", "bash", "wsl:", "wsl:--exec"] {
            assert!(kind(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn refuses_to_start_somewhere_else_than_the_project_folder() {
        let missing = std::env::temp_dir().join("lmux-no-such-folder");
        let error = command("cmd", missing.to_str().unwrap(), None).unwrap_err();
        assert!(error.contains("lmux-no-such-folder"), "{error}");
        assert!(command("nope", r"C:\Windows", None).is_err());
        let error = command("cmd", r"\\localhost\C$", None).unwrap_err();
        assert!(error.contains("cmd"), "{error}");
    }
}
