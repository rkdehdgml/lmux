import { invoke } from "@tauri-apps/api/core";
import type { Node } from "./layout";

export type Agent = "none" | "claude" | "codex" | "custom";
export type Kind = "svn" | "git" | "folder";

export type Project = {
  id: string;
  name: string;
  path: string;
  kind: Kind;
  defaultShell: string;
  defaultAgent: Agent;
};

export type SessionRecord = {
  id: string;
  projectId: string;
  name: string;
  shell: string;
  agent: Agent;
  customCommand: string;
};

export type SavedLayout = { id: string; name: string; tree: Node };
export type StoredState = {
  projects: Project[];
  sessions: SessionRecord[];
  layout: Node | null;
  savedLayouts: SavedLayout[];
};
export type Shell = { id: string; name: string };
export type Created = { id: string; state: StoredState };

export type NewProject = { name: string; path: string; shell: string; agent: Agent };
export type NewSession = {
  projectId: string;
  name: string;
  shell: string;
  agent: Agent;
  customCommand: string;
  prompt: string;
};

export const AGENTS: { id: Agent; name: string }[] = [
  { id: "claude", name: "Claude Code" },
  { id: "codex", name: "Codex" },
  { id: "custom", name: "직접 입력" },
  { id: "none", name: "셸만" },
];

export type TokenCounts = { input: number; output: number; cacheCreation: number; cacheRead: number };
export type WindowUsage = { today: TokenCounts; fiveHour: TokenCounts; sevenDay: TokenCounts };
export type AccountLimits = { fiveHour: number; weekly: number; estimated: boolean };
export type AccountSummary = { name: string; paths: string[]; usage: WindowUsage; limits: AccountLimits };

export type FileStatus = "M" | "A" | "D" | "?" | "C" | "!";
export type ChangedFile = {
  relPath: string;
  status: FileStatus;
  sessionIds: string[];
  isConflict: boolean;
};
export type ChangeSummary = {
  kind: Kind;
  revision: string | null;
  branch: string | null;
  files: ChangedFile[];
  addCount: number;
  delCount: number;
};
export type GitWorktreeInfo = {
  path: string;
  branch: string;
  isLocked: boolean;
};

export type HookLivePayload =
  | { type: "working" }
  | { type: "waiting"; request: string }
  | { type: "done" };

export type HookContextPayload = {
  used: number;
  size: number;
  estimated: boolean;
};

export type SessionStatePayload = {
  id: string;
  run: number;
  live: HookLivePayload | null;
  context: HookContextPayload | null;
};

export interface AppSettings {
  notifyWaiting: boolean;
  notifyDone: boolean;
  notifyError: boolean;
  notifyConflict: boolean;
  notifyContext: boolean;
  notifyContextThreshold: number;
  theme: string;
  language: string;
  density: string;
  terminalFont: string;
  terminalFontSize: number;
  terminalLineHeight: number;
  cursorShape: string;
  webgl: boolean;
  hideInactive: boolean;
  scrollbackLines: number;
  preventSleep: boolean;
  minimizeToTray: boolean;
  restoreSessions: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  notifyWaiting: true,
  notifyDone: true,
  notifyError: true,
  notifyConflict: true,
  notifyContext: true,
  notifyContextThreshold: 80,
  theme: "dark",
  language: "ko",
  density: "normal",
  terminalFont: "JetBrains Mono",
  terminalFontSize: 13,
  terminalLineHeight: 1.6,
  cursorShape: "block",
  webgl: true,
  hideInactive: true,
  scrollbackLines: 10000,
  preventSleep: true,
  minimizeToTray: true,
  restoreSessions: true,
};

export const api = {
  appState: () => invoke<{ state: StoredState; warning: string | null }>("app_state"),
  shells: (includeWsl: boolean) => invoke<Shell[]>("shells_list", { includeWsl }),
  detect: (path: string) => invoke<{ kind: Kind; name: string }>("project_detect", { path }),
  addProject: (project: NewProject) => invoke<Created>("project_add", project),
  removeProject: (id: string) => invoke<StoredState>("project_remove", { id }),
  createSession: (session: NewSession) => invoke<Created>("session_create", session),
  deleteSession: (id: string) => invoke<StoredState>("session_delete", { id }),
  start: (id: string) =>
    invoke<{ run: number; os_build: number }>("session_start", { id, cols: 80, rows: 24 }),
  kill: (id: string) => invoke("session_kill", { id }),
  setLayout: (tree: Node) => invoke<void>("layout_set", { tree }),
  saveLayout: (name: string, tree: Node) => invoke<StoredState>("layout_save", { name, tree }),
  deleteLayout: (id: string) => invoke<StoredState>("layout_delete", { id }),
  approve: (id: string, run: number, request: string, choice: 1 | 2 | 3) =>
    invoke<void>("session_approve", { id, run, request, choice }),
  accountList: () => invoke<AccountSummary[]>("account_list"),
  getChanges: (projectPath: string) =>
    invoke<ChangeSummary>("get_changes", { projectPath }),
  getDiff: (projectPath: string, filePath: string, isGit: boolean) =>
    invoke<string>("get_diff", { projectPath, filePath, isGit }),
  svnCommit: (projectPath: string, files: string[], message: string) =>
    invoke<string>("svn_commit_cmd", { projectPath, files, message }),
  svnRevert: (projectPath: string, files: string[]) =>
    invoke<string>("svn_revert_cmd", { projectPath, files }),
  svnUpdate: (projectPath: string) =>
    invoke<string>("svn_update_cmd", { projectPath }),
  gitCommit: (projectPath: string, files: string[], message: string) =>
    invoke<string>("git_commit_cmd", { projectPath, files, message }),
  gitWorktreeList: (projectPath: string) =>
    invoke<GitWorktreeInfo[]>("git_worktree_list_cmd", { projectPath }),
  gitWorktreeAdd: (projectPath: string, branch: string, worktreePath: string) =>
    invoke<string>("git_worktree_add_cmd", { projectPath, branch, worktreePath }),
  gitWorktreeRemove: (projectPath: string, worktreePath: string) =>
    invoke<string>("git_worktree_remove_cmd", { projectPath, worktreePath }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<void>("save_settings", { settings }),
  preventSleep: (enable: boolean) => invoke<void>("prevent_sleep", { enable }),
  updateLastRunning: (ids: string[]) => invoke<void>("update_last_running", { ids }),
  getLastRunning: () => invoke<string[]>("get_last_running"),
};
