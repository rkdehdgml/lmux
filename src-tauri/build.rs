// 앱 명령을 선언해 두면 capability에서 허용한 창만 부를 수 있다.
const COMMANDS: &[&str] = &[
    "app_state",
    "shells_list",
    "project_detect",
    "project_add",
    "project_remove",
    "session_create",
    "session_delete",
    "session_start",
    "session_attach",
    "session_detach",
    "session_ack",
    "session_write",
    "session_resize",
    "session_kill",
    "layout_set",
    "layout_save",
    "layout_delete",
];

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(COMMANDS);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri build failed");
}
