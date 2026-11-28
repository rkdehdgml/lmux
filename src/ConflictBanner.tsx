type Props = {
  files: string[];
  onOpenChanges: () => void;
};

export default function ConflictBanner({ files, onOpenChanges }: Props) {
  if (files.length === 0) return null;
  return (
    <div className="banner" role="alert">
      <svg style={{ width: 14, height: 14, flex: "none", fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" }} viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
      </svg>
      <span className="grow">
        <strong>{files.length}개</strong> 파일을 여러 세션이 동시에 고쳤어요.
      </span>
      <button className="btn" style={{ padding: "4px 8px", fontSize: 12 }} onClick={onOpenChanges}>
        변경 사항 보기
      </button>
    </div>
  );
}
