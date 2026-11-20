export type ChatFilter = 'all' | 'unread' | 'groups';

const ITEMS: Array<{ id: ChatFilter; label: string }> = [
  { id: 'all', label: 'All' },
  { id: 'unread', label: 'Unread' },
  { id: 'groups', label: 'Groups' },
];

/** All / Unread / Groups above the chat list: a segmented control whose highlight slides to the chosen one. */
export function FilterPills({ value, onChange, unread }: { value: ChatFilter; onChange: (v: ChatFilter) => void; unread: number }) {
  const index = ITEMS.findIndex((i) => i.id === value);
  return (
    <div className="filter-pills" role="tablist" aria-label="Filter chats" style={{ ['--i' as string]: index }}>
      <span className="filter-pills-thumb" aria-hidden="true" />
      {ITEMS.map((item) => (
        <button
          key={item.id}
          type="button"
          role="tab"
          aria-selected={value === item.id}
          className={`filter-pill ${value === item.id ? 'active' : ''}`}
          onClick={() => onChange(item.id)}
        >
          {item.label}
          {item.id === 'unread' && unread > 0 ? <span className="filter-pill-count">{unread}</span> : null}
        </button>
      ))}
    </div>
  );
}
