//! 앱 데이터 폴더의 JSON 상태(프로젝트·세션 기록). 사용자의 프로젝트 폴더에는 쓰지 않는다.

use crate::agent::Agent;
use crate::layout::{Node, SavedLayout};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FILE: &str = "state.json";
const LOCK_FILE: &str = "state.lock";
/// 다른 프로세스가 파일을 열고 있을 때의 Windows 오류 번호.
const SHARING_VIOLATION: i32 = 32;
const FOLDER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_STUCK: usize = 32;
const VERSION: u32 = 1;
const MAX_LAYOUTS: usize = 20;
const MAX_LAYOUT_NAME: usize = 40;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Svn,
    Git,
    Folder,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: Kind,
    pub default_shell: String,
    pub default_agent: Agent,
    /// 등록할 때 읽은 실제 경로. 같은 폴더를 다시 등록하는지 비교한다.
    #[serde(default)]
    pub identity: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub shell: String,
    pub agent: Agent,
    #[serde(default)]
    pub custom_command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // 알림
    pub notify_waiting: bool,
    pub notify_done: bool,
    pub notify_error: bool,
    pub notify_conflict: bool,
    pub notify_context: bool,
    pub notify_context_threshold: u8,
    // 화면
    pub theme: String,
    pub language: String,
    pub density: String,
    pub terminal_font: String,
    pub terminal_font_size: u8,
    pub terminal_line_height: f32,
    pub cursor_shape: String,
    // 성능
    pub webgl: bool,
    pub hide_inactive: bool,
    pub scrollback_lines: u32,
    pub prevent_sleep: bool,
    pub minimize_to_tray: bool,
    pub restore_sessions: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            notify_waiting: true,
            notify_done: true,
            notify_error: true,
            notify_conflict: true,
            notify_context: true,
            notify_context_threshold: 80,
            theme: "dark".into(),
            language: "ko".into(),
            density: "normal".into(),
            terminal_font: "JetBrains Mono".into(),
            terminal_font_size: 13,
            terminal_line_height: 1.6,
            cursor_shape: "block".into(),
            webgl: true,
            hide_inactive: true,
            scrollback_lines: 10000,
            prevent_sleep: true,
            minimize_to_tray: true,
            restore_sessions: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub version: u32,
    pub next_id: u64,
    pub projects: Vec<Project>,
    pub sessions: Vec<SessionRecord>,
    /// 마지막으로 본 화면 배치.
    #[serde(default)]
    pub layout: Option<Node>,
    #[serde(default)]
    pub saved_layouts: Vec<SavedLayout>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub last_running: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        State {
            version: VERSION,
            next_id: 1,
            projects: Vec::new(),
            sessions: Vec::new(),
            layout: None,
            saved_layouts: Vec::new(),
            settings: Settings::default(),
            last_running: Vec::new(),
        }
    }
}

/// 가장 가까운 상위 폴더의 `.svn`·`.git`으로 판단한다.
pub fn detect_kind(path: &Path) -> Kind {
    for dir in path.ancestors() {
        if dir.join(".svn").is_dir() {
            return Kind::Svn;
        }
        // 워크트리의 `.git`은 파일이다.
        if dir.join(".git").exists() {
            return Kind::Git;
        }
    }
    Kind::Folder
}

/// 등록하려는 폴더를 읽은 결과.
pub struct Folder {
    pub path: String,
    pub identity: String,
    pub kind: Kind,
}

/// 시간 초과한 뒤 아직 끝나지 않은 조회. 폴더마다 몇 개인지 센다.
type Stuck = Mutex<BTreeMap<String, usize>>;

/// `work`를 따로 실행해 `limit`까지만 기다린다. 응답 없는 네트워크 경로는 그대로 두고 돌아온다.
pub fn within<T: Send + 'static>(
    limit: Duration,
    stuck: &'static Stuck,
    path: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let key = path.trim_end_matches(['\\', '/']).to_lowercase();
    let list = move || stuck.lock().unwrap_or_else(PoisonError::into_inner);
    {
        // 막힌 조회는 OS가 포기할 때까지 남는다. 같은 폴더는 다시 읽지 않고, 전체 수에도 한도를 둔다.
        // 다른 폴더의 조회는 막지 않는다.
        let waiting = list();
        if waiting.contains_key(&key) || waiting.values().sum::<usize>() >= MAX_STUCK {
            return Err("이전 폴더 확인이 아직 끝나지 않았어요. 잠시 뒤 다시 시도하세요".into());
        }
    }
    let (done, result) = channel();
    thread::spawn(move || done.send(work()));
    match result.recv_timeout(limit) {
        Ok(value) => Ok(value),
        Err(_) => {
            *list().entry(key.clone()).or_default() += 1;
            // 끝날 때까지 기다렸다가 목록에서 뺀다.
            thread::spawn(move || {
                let _ = result.recv();
                let mut waiting = list();
                if let Some(count) = waiting.get_mut(&key) {
                    *count -= 1;
                    if *count == 0 {
                        waiting.remove(&key);
                    }
                }
            });
            Err("폴더가 응답하지 않아요. 연결을 확인해 주세요".into())
        }
    }
}

/// 등록하려는 폴더를 읽는다. 저장 상태를 잠그기 전에 부른다.
pub fn inspect(path: &str) -> Result<Folder, String> {
    let path = path.trim().to_string();
    static STUCK: Stuck = Mutex::new(BTreeMap::new());
    let key = path.clone();
    within(FOLDER_TIMEOUT, &STUCK, &key, move || read_folder(&path))?
}

fn read_folder(path: &str) -> Result<Folder, String> {
    let given = Path::new(path);
    if !given.is_absolute() || !given.is_dir() {
        return Err(format!("있는 폴더의 전체 경로를 골라 주세요: {path}"));
    }
    // 끝의 구분자와 `/`를 정리한다.
    let folder: PathBuf = given.components().collect();
    let path = folder.to_string_lossy().into_owned();
    Ok(Folder {
        identity: identity(&path),
        kind: detect_kind(&folder),
        path,
    })
}

pub struct Store {
    dir: PathBuf,
    state: State,
    /// 저장하면 기존 데이터를 잃는 상황이면 그 이유.
    locked: Option<String>,
    used_elsewhere: bool,
    /// 열어 둔 동안 다른 앱 인스턴스가 같은 데이터를 쓰지 못하게 하는 잠금.
    _lock: Option<fs::File>,
}

#[derive(Deserialize)]
struct Version {
    version: u32,
}

/// 읽은 상태, 저장을 막을 이유, 사용자에게 보여 줄 경고.
fn load(dir: &Path) -> (State, Option<String>, Option<String>) {
    let file = dir.join(FILE);
    let locked = |reason: String| (State::default(), Some(reason.clone()), Some(reason));
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (State::default(), None, None);
        }
        Err(error) => {
            return locked(format!(
                "저장 파일을 읽지 못해 변경을 저장하지 않아요: {} ({error})",
                file.display()
            ));
        }
    };
    if serde_json::from_str::<Version>(&text).is_ok_and(|v| v.version > VERSION) {
        return locked(format!(
            "더 새 버전의 Lmux가 만든 저장 파일이라 변경을 저장하지 않아요: {}",
            file.display()
        ));
    }
    match serde_json::from_str::<State>(&text) {
        Ok(mut state) => {
            // 그릴 수 없는 배치만 버리고 목록은 살린다.
            state.layout = state.layout.filter(|tree| tree.check().is_ok());
            state.saved_layouts.retain(|s| s.tree.check().is_ok());
            (state, None, None)
        }
        Err(error) => {
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let kept = dir.join(format!("{FILE}.broken-{seconds}"));
            match fs::rename(&file, &kept) {
                Ok(()) => (
                    State::default(),
                    None,
                    Some(format!(
                        "저장 파일이 깨져 빈 목록으로 시작해요. 원본: {} ({error})",
                        kept.display()
                    )),
                ),
                Err(rename) => locked(format!(
                    "깨진 저장 파일을 옮기지 못해 변경을 저장하지 않아요: {} ({rename})",
                    file.display()
                )),
            }
        }
    }
}

/// 다른 곳에서 쓰는 중인 파일은 옮기거나 고치지 않고 읽기만 한다.
fn load_readonly(dir: &Path, error: std::io::Error) -> (State, Option<String>, Option<String>) {
    let reason = if in_use(&error) {
        "다른 Lmux 창이 같은 데이터를 쓰고 있어요. 이 창은 다시 열기 전까지 저장과 세션 시작을 하지 않아요".into()
    } else {
        format!(
            "저장 폴더를 쓸 수 없어 변경을 저장하지 않아요: {} ({error})",
            dir.display()
        )
    };
    let state = fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<State>(&text).ok())
        .filter(|state| state.version <= VERSION)
        .map(|mut state| {
            state.layout = state.layout.filter(|tree| tree.check().is_ok());
            state.saved_layouts.retain(|s| s.tree.check().is_ok());
            state
        })
        .unwrap_or_default();
    (state, Some(reason.clone()), Some(reason))
}

fn in_use(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(SHARING_VIOLATION)
}

fn acquire(dir: &Path) -> std::io::Result<fs::File> {
    fs::create_dir_all(dir)?;
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(dir.join(LOCK_FILE))
}

fn write(dir: &Path, state: &State) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!("{FILE}.tmp"));
    let mut file = fs::File::create(&temp)?;
    file.write_all(&serde_json::to_vec_pretty(state)?)?;
    file.sync_all()?;
    drop(file);
    // 쓰다가 끊겨도 이전 파일이 남는다.
    fs::rename(temp, dir.join(FILE))
}

impl Store {
    /// 두 번째 값은 사용자에게 보여 줄 경고.
    pub fn open(dir: &Path) -> (Store, Option<String>) {
        let mut used_elsewhere = false;
        let (lock, (state, locked, warning)) = match acquire(dir) {
            Ok(lock) => (Some(lock), load(dir)),
            Err(error) => {
                used_elsewhere = in_use(&error);
                (None, load_readonly(dir, error))
            }
        };
        let store = Store {
            dir: dir.into(),
            state,
            locked,
            used_elsewhere,
            _lock: lock,
        };
        (store, warning)
    }

    /// 다른 앱 인스턴스가 이 데이터를 쓰고 있는지.
    pub fn used_elsewhere(&self) -> bool {
        self.used_elsewhere
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    /// 복사본을 바꿔 저장에 성공했을 때만 반영한다.
    fn change<T>(&mut self, f: impl FnOnce(&mut State) -> Result<T, String>) -> Result<T, String> {
        if let Some(reason) = &self.locked {
            return Err(reason.clone());
        }
        let mut next = self.state.clone();
        let value = f(&mut next)?;
        write(&self.dir, &next).map_err(|e| format!("저장하지 못했어요: {e}"))?;
        self.state = next;
        Ok(value)
    }

    pub fn add_project(
        &mut self,
        name: &str,
        folder: Folder,
        default_shell: &str,
        default_agent: Agent,
    ) -> Result<Project, String> {
        let Folder {
            path,
            identity,
            kind,
        } = folder;
        let folder = PathBuf::from(&path);
        let name = match name.trim() {
            "" => folder
                .file_name()
                .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned()),
            name => name.into(),
        };
        self.change(|state| {
            let lower = path.to_lowercase();
            // 식별값이 없는 예전 기록만 경로 글자로 비교한다.
            if let Some(same) = state.projects.iter().find(|p| {
                if p.identity.is_empty() {
                    p.path.to_lowercase() == lower
                } else {
                    p.identity == identity
                }
            }) {
                return Err(format!("이미 등록한 폴더예요: {}", same.name));
            }
            let project = Project {
                id: next_id(state, "p"),
                name,
                path,
                kind,
                default_shell: default_shell.into(),
                default_agent,
                identity,
            };
            state.projects.push(project.clone());
            Ok(project)
        })
    }

    /// 등록만 지운다. 함께 지운 세션 id를 돌려준다.
    pub fn remove_project(&mut self, id: &str) -> Result<Vec<String>, String> {
        self.change(|state| {
            let before = state.projects.len();
            state.projects.retain(|p| p.id != id);
            if state.projects.len() == before {
                return Err("프로젝트가 없어요".into());
            }
            let removed = state
                .sessions
                .iter()
                .filter(|s| s.project_id == id)
                .map(|s| s.id.clone())
                .collect();
            state.sessions.retain(|s| s.project_id != id);
            Ok(removed)
        })
    }

    pub fn add_session(
        &mut self,
        project_id: &str,
        name: &str,
        shell: &str,
        agent: Agent,
        custom_command: &str,
    ) -> Result<SessionRecord, String> {
        let custom = custom_command.trim();
        if agent == Agent::Custom && custom.is_empty() {
            return Err("실행할 명령을 입력해 주세요".into());
        }
        self.change(|state| {
            if !state.projects.iter().any(|p| p.id == project_id) {
                return Err("프로젝트가 없어요".into());
            }
            let id = next_id(state, "s");
            let name = match name.trim() {
                "" => {
                    let count = state
                        .sessions
                        .iter()
                        .filter(|s| s.project_id == project_id)
                        .count();
                    format!("세션 {}", count + 1)
                }
                name => name.into(),
            };
            let session = SessionRecord {
                id,
                project_id: project_id.into(),
                name,
                shell: shell.into(),
                agent,
                custom_command: if agent == Agent::Custom {
                    custom.into()
                } else {
                    String::new()
                },
            };
            state.sessions.push(session.clone());
            Ok(session)
        })
    }

    pub fn remove_session(&mut self, id: &str) -> Result<(), String> {
        self.change(|state| {
            let before = state.sessions.len();
            state.sessions.retain(|s| s.id != id);
            if state.sessions.len() == before {
                return Err("세션이 없어요".into());
            }
            Ok(())
        })
    }

    pub fn set_layout(&mut self, tree: Node) -> Result<(), String> {
        tree.check()?;
        self.change(|state| {
            state.layout = Some(tree);
            Ok(())
        })
    }

    /// 같은 이름이 있으면 덮어쓴다.
    pub fn save_layout(&mut self, name: &str, tree: Node) -> Result<(), String> {
        tree.check()?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_LAYOUT_NAME {
            return Err(format!("화면 이름은 1~{MAX_LAYOUT_NAME}자로 입력해 주세요"));
        }
        self.change(|state| {
            if let Some(same) = state.saved_layouts.iter_mut().find(|s| s.name == name) {
                same.tree = tree;
                return Ok(());
            }
            if state.saved_layouts.len() >= MAX_LAYOUTS {
                return Err(format!(
                    "화면은 {MAX_LAYOUTS}개까지 저장할 수 있어요. 안 쓰는 화면을 지워 주세요"
                ));
            }
            let id = next_id(state, "l");
            state.saved_layouts.push(SavedLayout {
                id,
                name: name.into(),
                tree,
            });
            Ok(())
        })
    }

    pub fn delete_layout(&mut self, id: &str) -> Result<(), String> {
        self.change(|state| {
            let before = state.saved_layouts.len();
            state.saved_layouts.retain(|s| s.id != id);
            if state.saved_layouts.len() == before {
                return Err("저장한 화면이 없어요".into());
            }
            Ok(())
        })
    }

    pub fn set_settings(&mut self, settings: Settings) -> Result<(), String> {
        self.change(|state| {
            state.settings = settings;
            Ok(())
        })
    }

    pub fn update_last_running(&mut self, ids: Vec<String>) -> Result<(), String> {
        self.change(|state| {
            state.last_running = ids;
            Ok(())
        })
    }
}

/// 같은 폴더인지 비교하는 값. 연결(junction)은 따라가고 대소문자는 디스크에 있는 대로 맞춘다.
fn identity(path: &str) -> String {
    fs::canonicalize(path).map_or_else(|_| path.into(), |real| real.to_string_lossy().into_owned())
}

fn next_id(state: &mut State, prefix: &str) -> String {
    let id = format!("{prefix}{}", state.next_id);
    state.next_id += 1;
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Dir;

    #[test]
    fn settings_default_values_are_sane() {
        let s = Settings::default();
        assert_eq!(s.theme, "dark");
        assert!(s.notify_waiting);
        assert!(s.prevent_sleep);
        assert_eq!(s.terminal_font_size, 13);
        assert_eq!(s.scrollback_lines, 10000);
    }

    #[test]
    fn settings_roundtrip_json() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.theme, s.theme);
        assert_eq!(back.terminal_font_size, s.terminal_font_size);
    }

    #[test]
    fn last_running_persists_in_state() {
        let mut s = State::default();
        s.last_running = vec!["s1".to_string()];
        let json = serde_json::to_string(&s).unwrap();
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(back.last_running, vec!["s1"]);
    }

    /// 테스트마다 새 폴더를 쓰고 끝나면 지운다.
    struct Temp(PathBuf);

    impl Temp {
        fn new(name: &str) -> Temp {
            let dir =
                std::env::temp_dir().join(format!("lmux-store-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Temp(dir)
        }

        fn sub(&self, path: &str) -> PathBuf {
            let dir = self.0.join(path);
            fs::create_dir_all(&dir).unwrap();
            dir
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn add(store: &mut Store, path: &Path) -> Result<Project, String> {
        store.add_project("이름", inspect(path.to_str().unwrap())?, "cmd", Agent::None)
    }

    #[test]
    fn detects_the_nearest_version_control() {
        let temp = Temp::new("kind");
        temp.sub("svn/.svn");
        temp.sub("svn/src");
        temp.sub("svn/inner/.git");
        temp.sub("both/.svn");
        temp.sub("both/.git");
        temp.sub("plain");
        temp.sub("worktree");
        fs::write(temp.0.join("worktree/.git"), "gitdir: x").unwrap();
        for (path, kind) in [
            ("svn", Kind::Svn),
            ("svn/src", Kind::Svn),
            ("svn/inner", Kind::Git),
            ("both", Kind::Svn),
            ("worktree", Kind::Git),
            ("plain", Kind::Folder),
        ] {
            assert_eq!(detect_kind(&temp.0.join(path)), kind, "{path}");
        }
    }

    #[test]
    fn keeps_projects_and_sessions_after_reopening() {
        let temp = Temp::new("reopen");
        let data = temp.0.join("data");
        let work = temp.sub("work/.git").parent().unwrap().to_path_buf();
        let (mut store, warning) = Store::open(&data);
        assert!(warning.is_none());
        let trailing = format!(r"{}\", work.display());
        let project = store
            .add_project("  ", inspect(&trailing).unwrap(), "cmd", Agent::Claude)
            .unwrap();
        assert_eq!(project.name, "work", "빈 이름은 폴더 이름");
        assert_eq!(project.path, work.to_str().unwrap());
        assert_eq!(project.kind, Kind::Git);
        let first = store
            .add_session(&project.id, "", "cmd", Agent::None, "")
            .unwrap();
        let second = store
            .add_session(&project.id, "둘", "gitbash", Agent::Custom, "aider")
            .unwrap();
        assert_eq!(first.name, "세션 1");
        assert_ne!(first.id, second.id);

        drop(store);
        let (mut store, warning) = Store::open(&data);
        assert!(warning.is_none());
        assert_eq!(store.state().projects.len(), 1);
        assert_eq!(store.state().sessions[1].custom_command, "aider");
        let third = store
            .add_session(&project.id, "", "cmd", Agent::None, "")
            .unwrap();
        assert!(
            ![first.id, second.id].contains(&third.id),
            "id를 다시 쓰지 않는다"
        );
        assert!(!data.join("state.json.tmp").exists());
    }

    #[test]
    fn rejects_invalid_projects_and_sessions() {
        let temp = Temp::new("reject");
        let work = temp.sub("Work");
        let (mut store, _) = Store::open(&temp.0.join("data"));
        assert!(add(&mut store, &temp.0.join("none")).is_err());
        assert!(add(&mut store, Path::new("relative")).is_err());
        let project = add(&mut store, &work).unwrap();
        let upper = work.to_str().unwrap().to_uppercase();
        assert!(add(&mut store, Path::new(&upper)).is_err(), "같은 폴더");
        assert!(
            store
                .add_session("p999", "", "cmd", Agent::None, "")
                .is_err()
        );
        assert!(
            store
                .add_session(&project.id, "", "cmd", Agent::Custom, "  ")
                .is_err()
        );
        assert_eq!(store.state().sessions.len(), 0);
    }

    #[test]
    fn removing_a_project_forgets_its_sessions_but_keeps_the_files() {
        let temp = Temp::new("remove");
        let work = temp.sub("a");
        fs::write(work.join("keep.txt"), "x").unwrap();
        let other = temp.sub("b");
        let (mut store, _) = Store::open(&temp.0.join("data"));
        let a = add(&mut store, &work).unwrap();
        let b = add(&mut store, &other).unwrap();
        let s1 = store
            .add_session(&a.id, "", "cmd", Agent::None, "")
            .unwrap();
        let s2 = store
            .add_session(&b.id, "", "cmd", Agent::None, "")
            .unwrap();
        assert_eq!(store.remove_project(&a.id).unwrap(), [s1.id]);
        assert_eq!(store.state().sessions.len(), 1);
        assert_eq!(fs::read_dir(&work).unwrap().count(), 1);
        store.remove_session(&s2.id).unwrap();
        assert!(store.remove_session(&s2.id).is_err());
    }

    #[test]
    fn preserves_a_broken_file_and_says_so() {
        let temp = Temp::new("broken");
        let data = temp.sub("data");
        fs::write(data.join(FILE), "{ not json").unwrap();
        let (mut store, warning) = Store::open(&data);
        assert!(warning.unwrap().contains("broken"));
        let kept = fs::read_dir(&data)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.to_string_lossy().contains(".broken-"))
            .expect("원본 보존");
        assert_eq!(fs::read_to_string(kept).unwrap(), "{ not json");
        add(&mut store, &temp.sub("w")).unwrap();
    }

    #[test]
    fn never_overwrites_a_newer_file() {
        let temp = Temp::new("newer");
        let data = temp.sub("data");
        let newer = r#"{"version":99,"somethingNew":true}"#;
        fs::write(data.join(FILE), newer).unwrap();
        let (mut store, warning) = Store::open(&data);
        assert!(warning.is_some());
        assert!(add(&mut store, &temp.sub("w")).is_err());
        assert_eq!(fs::read_to_string(data.join(FILE)).unwrap(), newer);
    }

    #[test]
    fn only_one_open_store_may_write() {
        let temp = Temp::new("lock");
        let data = temp.0.join("data");
        let (mut first, _) = Store::open(&data);
        add(&mut first, &temp.sub("a")).unwrap();
        let (mut second, warning) = Store::open(&data);
        assert!(warning.unwrap().contains("다른 Lmux"));
        assert_eq!(second.state().projects.len(), 1, "목록은 보여 준다");
        assert!(add(&mut second, &temp.sub("b")).is_err());
        drop(first);
        let (mut third, warning) = Store::open(&data);
        assert!(warning.is_none());
        add(&mut third, &temp.sub("b")).unwrap();
    }

    #[test]
    fn rejects_the_same_folder_reached_through_a_junction() {
        let temp = Temp::new("junction");
        let real = temp.sub("real");
        let link = temp.0.join("link");
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .args([&link, &real])
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        let (mut store, _) = Store::open(&temp.0.join("data"));
        add(&mut store, &real).unwrap();
        let error = add(&mut store, &link).unwrap_err();
        // 폴더를 지우기 전에 연결부터 지운다.
        fs::remove_dir(&link).unwrap();
        assert!(error.contains("이미 등록"), "{error}");
    }

    /// 이미 등록한 폴더는 다시 읽지 않고 저장해 둔 실제 경로로 비교한다.
    #[test]
    fn compares_with_the_saved_identity_without_reading_old_folders() {
        let temp = Temp::new("identity");
        let real = temp.sub("real");
        let link = temp.0.join("link");
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .args([&link, &real])
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        let (mut store, _) = Store::open(&temp.0.join("data"));
        add(&mut store, &link).unwrap();
        // 등록한 경로를 지금은 읽을 수 없어도 같은 폴더인지 안다.
        fs::remove_dir(&link).unwrap();
        assert!(add(&mut store, &real).unwrap_err().contains("이미 등록"));
    }

    fn slow(stuck: &'static Stuck, path: &str) -> Result<(), String> {
        within(Duration::from_millis(20), stuck, path, || {
            std::thread::sleep(Duration::from_millis(600));
        })
    }

    #[test]
    fn gives_up_on_a_folder_that_does_not_answer() {
        static STUCK: Stuck = Mutex::new(BTreeMap::new());
        let team = r"\\srv\dfs\team";
        assert!(slow(&STUCK, team).unwrap_err().contains("응답"));
        // 아직 끝나지 않은 같은 폴더는 다시 읽지 않는다(대소문자·끝 구분자 무시).
        let started = std::time::Instant::now();
        assert!(
            slow(&STUCK, r"\\SRV\dfs\Team\")
                .unwrap_err()
                .contains("이전")
        );
        assert!(started.elapsed() < Duration::from_millis(15));
        // 같은 공유의 다른 폴더는 막지 않는다.
        let other = within(Duration::from_secs(2), &STUCK, r"\\srv\dfs\other", || 2);
        assert_eq!(other.unwrap(), 2);
        // 끝나면 다시 받는다.
        std::thread::sleep(Duration::from_millis(800));
        assert_eq!(
            within(Duration::from_secs(2), &STUCK, team, || 3).unwrap(),
            3
        );
    }

    #[test]
    fn stops_piling_up_reads_that_never_finish() {
        static STUCK: Stuck = Mutex::new(BTreeMap::new());
        // 목록을 채우는 동안 먼저 만든 조회가 끝나지 않도록 오래 걸리게 한다.
        let hang = |path: &str| {
            within(Duration::from_millis(5), &STUCK, path, || {
                std::thread::sleep(Duration::from_secs(5));
            })
        };
        for n in 0..MAX_STUCK {
            assert!(hang(&format!("folder-{n}")).unwrap_err().contains("응답"));
        }
        assert!(hang("one-more").unwrap_err().contains("이전"));
    }

    #[test]
    fn accepts_any_number_of_reads_that_finish_in_time() {
        static STUCK: Stuck = Mutex::new(BTreeMap::new());
        thread::scope(|scope| {
            for _ in 0..40 {
                scope.spawn(|| {
                    let quick = within(Duration::from_secs(5), &STUCK, "same", || {
                        std::thread::sleep(Duration::from_millis(100));
                    });
                    assert!(quick.is_ok(), "{quick:?}");
                });
            }
        });
    }

    /// 저장한 식별값이 서로 다르면 경로의 대소문자만 같아도 다른 폴더다.
    #[test]
    fn compares_paths_by_letters_only_for_records_without_an_identity() {
        let temp = Temp::new("case");
        let (mut store, _) = Store::open(&temp.0.join("data"));
        let folder = |path: &str, identity: &str| Folder {
            path: path.into(),
            identity: identity.into(),
            kind: Kind::Folder,
        };
        let add = |store: &mut Store, f| store.add_project("n", f, "cmd", Agent::None);
        add(&mut store, folder(r"C:\x\Repo", r"\\?\C:\x\Repo")).unwrap();
        add(&mut store, folder(r"C:\x\repo", r"\\?\C:\x\repo")).unwrap();
        add(&mut store, folder(r"C:\x\old", "")).unwrap();
        assert!(add(&mut store, folder(r"C:\x\OLD", r"\\?\C:\x\OLD")).is_err());
    }

    #[test]
    fn a_failed_save_leaves_memory_unchanged() {
        let temp = Temp::new("fail");
        // 데이터 폴더 자리에 파일이 있으면 저장할 수 없다.
        let data = temp.0.join("data");
        fs::write(&data, "x").unwrap();
        let (mut store, _) = Store::open(&data);
        assert!(add(&mut store, &temp.sub("w")).is_err());
        assert!(store.state().projects.is_empty());
    }

    fn pane(session: &str) -> Node {
        Node::Pane {
            session: Some(session.into()),
        }
    }

    fn split(ratio: f64, first: Node, second: Node) -> Node {
        Node::Split {
            dir: Dir::Row,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[test]
    fn keeps_the_layout_and_saved_layouts_after_reopening() {
        let temp = Temp::new("layout");
        let tree = split(0.6, pane("s1"), pane("s2"));
        {
            let (mut store, _) = Store::open(&temp.0);
            store.set_layout(tree.clone()).unwrap();
            store.save_layout(" 둘 ", pane("s1")).unwrap();
            store.save_layout("셋", tree.clone()).unwrap();
            // 같은 이름은 덮어쓴다.
            store.save_layout("둘", tree.clone()).unwrap();
        }
        let (store, warning) = Store::open(&temp.0);
        assert_eq!(warning, None);
        assert_eq!(store.state().layout.as_ref(), Some(&tree));
        let saved = &store.state().saved_layouts;
        assert_eq!(
            saved.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["둘", "셋"]
        );
        assert_eq!(saved[0].tree, tree);
    }

    #[test]
    fn deletes_a_saved_layout() {
        let temp = Temp::new("layout-delete");
        let (mut store, _) = Store::open(&temp.0);
        store.save_layout("a", pane("s1")).unwrap();
        let id = store.state().saved_layouts[0].id.clone();
        store.delete_layout(&id).unwrap();
        assert!(store.state().saved_layouts.is_empty());
        assert!(store.delete_layout(&id).is_err());
    }

    #[test]
    fn rejects_layouts_the_screen_cannot_show() {
        let temp = Temp::new("layout-reject");
        let (mut store, _) = Store::open(&temp.0);
        let five = (0..4).fold(pane("s0"), |tree, _| split(0.5, tree, pane("s")));
        assert!(store.set_layout(five.clone()).is_err());
        for ratio in [0.1, 0.9, f64::NAN] {
            assert!(
                store
                    .set_layout(split(ratio, pane("a"), pane("b")))
                    .is_err(),
                "{ratio}"
            );
        }
        assert!(store.save_layout("a", five).is_err());
        assert!(store.save_layout("  ", pane("a")).is_err());
        assert!(store.save_layout(&"가".repeat(41), pane("a")).is_err());
        for n in 0..20 {
            store.save_layout(&format!("{n}"), pane("a")).unwrap();
        }
        assert!(store.save_layout("21", pane("a")).is_err());
        assert!(store.save_layout("0", pane("b")).is_ok());
        assert_eq!(store.state().layout, None);
    }

    #[test]
    fn opens_a_file_saved_before_layouts_existed() {
        let temp = Temp::new("layout-old");
        fs::write(
            temp.0.join(FILE),
            r#"{"version":1,"nextId":3,"projects":[],"sessions":[]}"#,
        )
        .unwrap();
        let (store, warning) = Store::open(&temp.0);
        assert_eq!(warning, None);
        assert_eq!(store.state().next_id, 3);
        assert_eq!(store.state().layout, None);
        assert!(store.state().saved_layouts.is_empty());
    }

    #[test]
    fn drops_only_the_layouts_it_cannot_show() {
        let temp = Temp::new("layout-bad");
        let bad = r#"{"type":"split","dir":"row","ratio":0.99,
            "first":{"type":"pane","session":null},"second":{"type":"pane","session":null}}"#;
        let good = r#"{"type":"pane","session":"s1"}"#;
        fs::write(
            temp.0.join(FILE),
            format!(
                r#"{{"version":1,"nextId":9,"projects":[],"sessions":[],"layout":{bad},
                "savedLayouts":[{{"id":"l1","name":"a","tree":{bad}}},{{"id":"l2","name":"b","tree":{good}}}]}}"#
            ),
        )
        .unwrap();
        let (store, _) = Store::open(&temp.0);
        // 다른 창이 쓰는 중이라 읽기만 할 때도 같다.
        let (second, _) = Store::open(&temp.0);
        assert!(second.used_elsewhere());
        assert_eq!(second.state().layout, None);
        assert_eq!(second.state().saved_layouts.len(), 1);
        assert_eq!(store.state().next_id, 9);
        assert_eq!(store.state().layout, None);
        assert_eq!(store.state().saved_layouts.len(), 1);
        assert_eq!(store.state().saved_layouts[0].id, "l2");
    }

    #[test]
    fn reads_the_layout_shape_the_screen_sends() {
        let sent = r#"{"type":"split","dir":"col","ratio":0.5,
            "first":{"type":"pane","session":"s1"},"second":{"type":"pane","session":null}}"#;
        let tree: Node = serde_json::from_str(sent).unwrap();
        assert_eq!(
            serde_json::to_value(&tree).unwrap(),
            serde_json::from_str::<serde_json::Value>(sent).unwrap()
        );
    }
}
