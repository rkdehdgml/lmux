import { useCallback, useEffect, useRef, useState } from "react";
import { api, type ChangedFile, type ChangeSummary, type GitWorktreeInfo, type Project, type SessionRecord } from "./api";

type Props = {
  project: Project;
  sessions: SessionRecord[];
  onClose: () => void;
};

type SessionFilter = "all" | string;

const STATUS_LABEL: Record<string, string> = {
  M: "M", A: "A", D: "D", "?": "?", C: "C", "!": "!",
};

const STATUS_COLOR: Record<string, string> = {
  M: "#F2B54A", A: "#4CC79A", D: "#F27A82", "?": "#6EA8FF", C: "#FF6B6B", "!": "#FF9800",
};

function KindBadge({ kind }: { kind: "svn" | "git" | "folder" }) {
  const color = kind === "svn" ? "#2A2140" : "#1A2F20";
  const text = kind === "svn" ? "SVN" : "Git";
  const textColor = kind === "svn" ? "#C3A8FF" : "#4CC79A";
  if (kind === "folder") return null;
  return (
    <span style={{ background: color, color: textColor, fontSize: "10.5px", fontWeight: 600, borderRadius: 5, padding: "1px 6px", lineHeight: "16px" }}>
      {text}
    </span>
  );
}

function SessionBadge({ sessionIds, sessions }: { sessionIds: string[]; sessions: SessionRecord[] }) {
  if (sessionIds.length === 0) return null;
  if (sessionIds.length === 1) {
    const name = sessions.find(s => s.id === sessionIds[0])?.name ?? sessionIds[0];
    const short = name.length > 6 ? name.slice(0, 6) + "…" : name;
    return (
      <span style={{ background: "#1C2130", color: "#8DBAFF", fontSize: "10.5px", borderRadius: 5, padding: "1px 6px", lineHeight: "16px", fontFamily: "inherit" }}>
        {short}
      </span>
    );
  }
  return (
    <span style={{ background: "#3A2C0C", color: "#F2B54A", fontSize: "10.5px", borderRadius: 5, padding: "1px 6px", lineHeight: "16px", fontFamily: "inherit" }}>
      {sessionIds.length}개 세션
    </span>
  );
}

function FileLine({
  file,
  sessions,
  selected,
  checked,
  onSelect,
  onCheck,
}: {
  file: ChangedFile;
  sessions: SessionRecord[];
  selected: boolean;
  checked: boolean;
  onSelect: () => void;
  onCheck: (v: boolean) => void;
}) {
  const bg = selected ? "#1C2130" : file.isConflict ? "#221B0E" : "transparent";
  return (
    <label
      className="changes-file-row"
      style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 10px", fontSize: 13, borderRadius: 8, background: bg, cursor: "pointer" }}
      onClick={onSelect}
    >
      <input
        type="checkbox"
        checked={checked}
        style={{ accentColor: "#6EA8FF" }}
        onChange={e => { e.stopPropagation(); onCheck(e.target.checked); }}
        onClick={e => e.stopPropagation()}
        aria-label={`${file.relPath} 커밋에 포함`}
      />
      <span className="mono" style={{ color: STATUS_COLOR[file.status] ?? "#A0A8B8", fontSize: 11 }}>
        {STATUS_LABEL[file.status] ?? file.status}
      </span>
      <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {file.relPath}
      </span>
      <SessionBadge sessionIds={file.sessionIds} sessions={sessions} />
    </label>
  );
}

export default function ChangesPanel({ project, sessions, onClose }: Props) {
  const [summary, setSummary] = useState<ChangeSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [diff, setDiff] = useState<string>("");
  const [diffLoading, setDiffLoading] = useState(false);
  const [checkedFiles, setCheckedFiles] = useState<Set<string>>(new Set());
  const [commitMsg, setCommitMsg] = useState("");
  const [filter, setFilter] = useState<SessionFilter>("all");
  const [worktrees, setWorktrees] = useState<GitWorktreeInfo[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const toastRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    if (toastRef.current) clearTimeout(toastRef.current);
    toastRef.current = setTimeout(() => setToast(null), 3000);
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await api.getChanges(project.path);
      setSummary(s);
      setCheckedFiles(new Set(s.files.map(f => f.relPath)));
      if (project.kind === "git") {
        const wt = await api.gitWorktreeList(project.path);
        setWorktrees(wt);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [project]);

  useEffect(() => { load(); }, [load]);
  useEffect(() => () => { if (toastRef.current) clearTimeout(toastRef.current); }, []);

  const selectFile = useCallback(async (relPath: string) => {
    setSelectedFile(relPath);
    setDiffLoading(true);
    try {
      const d = await api.getDiff(project.path, relPath, project.kind === "git");
      setDiff(d);
    } catch {
      setDiff("");
    } finally {
      setDiffLoading(false);
    }
  }, [project]);

  const toggleCheck = useCallback((relPath: string, v: boolean) => {
    setCheckedFiles(prev => {
      const next = new Set(prev);
      if (v) next.add(relPath); else next.delete(relPath);
      return next;
    });
  }, []);

  const handleCommit = useCallback(async () => {
    if (!commitMsg.trim()) { showToast("커밋 메시지를 입력해 주세요."); return; }
    const files = [...checkedFiles];
    if (files.length === 0) { showToast("커밋할 파일을 선택해 주세요."); return; }
    if (!window.confirm(`${files.length}개 파일을 커밋할까요?`)) return;
    try {
      if (project.kind === "svn") {
        await api.svnCommit(project.path, files, commitMsg);
      } else {
        await api.gitCommit(project.path, files, commitMsg);
      }
      showToast("커밋 완료!");
      setCommitMsg("");
      load();
    } catch (e) {
      showToast(String(e));
    }
  }, [project, checkedFiles, commitMsg, load, showToast]);

  const handleRevert = useCallback(async () => {
    if (project.kind !== "svn") return;
    const files = [...checkedFiles];
    if (files.length === 0) { showToast("되돌릴 파일을 선택해 주세요."); return; }
    if (!window.confirm(`${files.length}개 파일을 되돌릴까요?`)) return;
    try {
      await api.svnRevert(project.path, files);
      showToast("되돌리기 완료!");
      load();
    } catch (e) {
      showToast(String(e));
    }
  }, [project, checkedFiles, load, showToast]);

  const handleUpdate = useCallback(async () => {
    try {
      const r = await api.svnUpdate(project.path);
      showToast(r || "svn update 완료!");
      load();
    } catch (e) {
      showToast(String(e));
    }
  }, [project, load, showToast]);

  const visibleFiles = summary?.files.filter(f =>
    filter === "all" || f.sessionIds.includes(filter)
  ) ?? [];

  const checkedCount = [...checkedFiles].filter(p => visibleFiles.some(f => f.relPath === p)).length;

  const revisionLine = summary
    ? summary.kind === "svn"
      ? `작업 사본 ${summary.revision ?? ""}`
      : summary.branch
        ? `브랜치 ${summary.branch}`
        : ""
    : "";

  const uniqueSessions = Array.from(
    new Set(summary?.files.flatMap(f => f.sessionIds) ?? [])
  ).map(id => sessions.find(s => s.id === id)).filter(Boolean) as SessionRecord[];

  return (
    <div className="changes-overlay" role="dialog" aria-labelledby="mg-title">
      <div className="changes-panel">
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "20px 24px", flexWrap: "wrap" }}>
          <div style={{ flex: 1, minWidth: 240 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <h2 id="mg-title" style={{ margin: 0, fontSize: 18, fontWeight: 700 }}>
                변경 사항 · {project.name}
              </h2>
              <KindBadge kind={project.kind} />
            </div>
            {!loading && summary && (
              <div className="muted" style={{ fontSize: "12.5px", marginTop: 3 }}>
                {revisionLine} · 파일 {summary.files.length}개&nbsp;
                <span className="mono" style={{ color: "#4CC79A" }}>+{summary.addCount}</span>&nbsp;
                <span className="mono" style={{ color: "#F27A82" }}>−{summary.delCount}</span>
              </div>
            )}
          </div>

          {/* Session filter tabs */}
          {uniqueSessions.length > 0 && (
            <div role="group" aria-label="세션별 보기" style={{ display: "flex", gap: 4, background: "#0E1014", borderRadius: 9, padding: 3 }}>
              <button
                className="btn"
                style={{ padding: "6px 10px", background: filter === "all" ? "#262C38" : "transparent", color: filter === "all" ? undefined : "#A0A8B8" }}
                onClick={() => setFilter("all")}
              >
                전체
              </button>
              {uniqueSessions.map(s => (
                <button
                  key={s.id}
                  className="btn"
                  style={{ padding: "6px 10px", background: filter === s.id ? "#262C38" : "transparent", color: filter === s.id ? undefined : "#A0A8B8" }}
                  onClick={() => setFilter(s.id)}
                >
                  {s.name}
                </button>
              ))}
            </div>
          )}

          <button className="btn" style={{ background: "transparent", padding: 6 }} onClick={onClose} aria-label="닫기">
            <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div style={{ display: "flex", flexWrap: "wrap", minHeight: 420, gap: 0, margin: "0 16px", background: "#0E1014", borderRadius: 12, overflow: "hidden" }}>
          {/* Left: file list */}
          <div style={{ flex: "1 1 300px", maxWidth: 340, padding: "10px 8px", display: "flex", flexDirection: "column", gap: 2, background: "#111318" }}>
            {loading ? (
              <div role="status" style={{ padding: "24px 10px", color: "#A0A8B8", fontSize: 13, textAlign: "center" }}>
                불러오는 중…
              </div>
            ) : error ? (
              <div style={{ padding: "12px 10px", color: "#F27A82", fontSize: 13 }}>{error}</div>
            ) : visibleFiles.length === 0 ? (
              <div className="dim" style={{ padding: "12px 10px", fontSize: 13 }}>변경된 파일이 없어요.</div>
            ) : (
              visibleFiles.map(f => (
                <FileLine
                  key={f.relPath}
                  file={f}
                  sessions={sessions}
                  selected={selectedFile === f.relPath}
                  checked={checkedFiles.has(f.relPath)}
                  onSelect={() => selectFile(f.relPath)}
                  onCheck={v => toggleCheck(f.relPath, v)}
                />
              ))
            )}
            <div className="dim" style={{ fontSize: "11.5px", padding: "10px 10px 0", lineHeight: "18px" }}>
              세션이 고친 파일을 자동으로 표시해요. 체크한 파일만 커밋돼요.
            </div>
          </div>

          {/* Right: diff viewer */}
          <div style={{ flex: "999 1 480px", minWidth: 0, overflow: "auto" }}>
            {selectedFile && (
              <div style={{ padding: "12px 18px", fontSize: 12, display: "flex", alignItems: "center", gap: 8, fontFamily: "var(--mono)", color: "#A0A8B8" }}>
                {selectedFile}
              </div>
            )}
            {diffLoading ? (
              <div role="status" style={{ padding: "24px 18px", color: "#A0A8B8", fontSize: 13 }}>불러오는 중…</div>
            ) : diff ? (
              <pre className="changes-diff-code">{diff}</pre>
            ) : (
              <div className="dim" style={{ padding: "24px 18px", fontSize: 13 }}>파일을 클릭하면 diff가 표시돼요.</div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div style={{ display: "flex", flexDirection: "column", gap: 10, padding: "16px 24px 22px" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <label htmlFor="mg-msg" style={{ fontSize: 13, fontWeight: 500, color: "#C7CCD6", flex: 1 }}>커밋 메시지</label>
          </div>
          <textarea
            id="mg-msg"
            className="mono"
            rows={2}
            value={commitMsg}
            onChange={e => setCommitMsg(e.target.value)}
            placeholder="커밋 메시지를 입력하세요"
            style={{ fontSize: 13, background: "#0E1014", border: "1px solid #262B36", borderRadius: 9, color: "#E8EAF0", padding: "10px 12px", resize: "none", fontFamily: "var(--mono)" }}
          />
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            {project.kind === "svn" && (
              <span className="dim" style={{ fontSize: 12, flex: 1 }}>
                커밋 전에 <span className="mono">svn update</span>로 서버 변경을 먼저 받아요
              </span>
            )}
            {project.kind === "svn" && (
              <>
                <button className="btn" onClick={handleUpdate}>업데이트 (svn update)</button>
                <button className="btn" onClick={handleRevert}>되돌리기 (svn revert)</button>
              </>
            )}
            <button className="btn primary" onClick={handleCommit}>
              {project.kind === "svn" ? `파일 ${checkedCount}개 SVN 커밋` : `파일 ${checkedCount}개 Git 커밋`}
            </button>
          </div>

          {/* Git worktree section */}
          {project.kind === "git" && worktrees.length > 0 && (
            <div style={{ borderTop: "1px solid #20252F", paddingTop: 10 }}>
              <div className="dim" style={{ fontSize: "11.5px", marginBottom: 6 }}>Git 워크트리</div>
              {worktrees.map(wt => (
                <div key={wt.path} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 12, padding: "4px 0" }}>
                  <span className="mono muted">{wt.branch}</span>
                  <span className="dim" style={{ flex: 1, fontSize: "11px" }}>{wt.path}</span>
                  {wt.isLocked && <span style={{ color: "#F2B54A", fontSize: "10.5px" }}>잠김</span>}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Toast */}
        {toast && (
          <div role="alert" style={{ position: "absolute", bottom: 20, left: "50%", transform: "translateX(-50%)", background: "#1F242E", color: "#E8EAF0", padding: "8px 16px", borderRadius: 8, fontSize: 13, boxShadow: "0 4px 16px #000A" }}>
            {toast}
          </div>
        )}
      </div>
    </div>
  );
}
