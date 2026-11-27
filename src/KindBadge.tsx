import type { Kind } from "./api";

export const KIND_NAMES: Record<Kind, { badge: string; description: string }> = {
  svn: { badge: "SVN", description: "SVN 작업 사본이에요" },
  git: { badge: "Git", description: "Git 저장소예요" },
  folder: { badge: "폴더", description: "버전 관리 없는 폴더예요" },
};

export function KindBadge({ kind }: { kind: Kind }) {
  return <span className={`badge ${kind}`}>{KIND_NAMES[kind].badge}</span>;
}
