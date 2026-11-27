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
};
