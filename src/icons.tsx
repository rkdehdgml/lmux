function Icon({ d }: { d: string }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

export const PlusIcon = () => <Icon d="M12 5v14M5 12h14" />;
export const CloseIcon = () => <Icon d="M6 6l12 12M18 6L6 18" />;
export const FolderIcon = () => (
  <Icon d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
);
export const TrashIcon = () => (
  <Icon d="M4 7h16M9 7V5h6v2M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M10 11v6M14 11v6" />
);

const LAYOUTS = {
  1: "",
  2: "M12 4v16",
  3: "M12 4v16M12 12h8",
  4: "M12 4v16M4 12h16",
};
export const LayoutIcon = ({ panes }: { panes: 1 | 2 | 3 | 4 }) => (
  <Icon d={`M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z${LAYOUTS[panes]}`} />
);
export const SwapIcon = () => <Icon d="M7 4v16M17 4v16M4 7h3M17 17h3" />;

export function Logo() {
  return (
    <span className="logo">
      <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="3" width="8" height="8" rx="2" />
        <rect x="13" y="3" width="8" height="8" rx="2" />
        <rect x="3" y="13" width="8" height="8" rx="2" />
        <rect x="13" y="13" width="8" height="8" rx="2" />
      </svg>
    </span>
  );
}
