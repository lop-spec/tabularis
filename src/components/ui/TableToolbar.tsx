import React, { useState } from "react";
import { Filter, ArrowUpDown, ListFilter } from "lucide-react";

interface TableToolbarProps {
  initialFilter?: string;
  initialSort?: string;
  initialLimit?: number | null;
  placeholderColumn: string;
  placeholderSort: string;
  defaultLimit: number;
  onUpdate: (filter: string, sort: string, limit: number | undefined) => void;
}

// Internal component that resets when key changes
const TableToolbarInternal: React.FC<TableToolbarProps> = ({
  initialFilter,
  initialSort,
  initialLimit,
  placeholderColumn,
  placeholderSort,
  defaultLimit,
  onUpdate,
}) => {
  // Local state to isolate typing from parent re-renders
  const [filterInput, setFilterInput] = useState(initialFilter || "");
  const [sortInput, setSortInput] = useState(initialSort || "");
  const [limitInput, setLimitInput] = useState(
    initialLimit && initialLimit > 0 ? String(initialLimit) : ""
  );

  const commitChanges = () => {
    const limitVal = limitInput ? parseInt(limitInput) : undefined;
    
    // Check if values have actually changed compared to initial props
    // We treat undefined and empty string as equivalent for comparison
    const filterChanged = (filterInput || "") !== (initialFilter || "");
    const sortChanged = (sortInput || "") !== (initialSort || "");
    const limitChanged = limitVal !== initialLimit;

    if (filterChanged || sortChanged || limitChanged) {
        onUpdate(filterInput, sortInput, limitVal);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      commitChanges();
    }
  };

  return (
    <div className="h-10 bg-elevated border-y border-default flex items-center px-2 gap-4">
      <div className="flex items-center gap-2 flex-1 bg-base border border-default rounded px-2 py-1 focus-within:border-blue-500/50 transition-colors">
        <Filter size={14} className="text-muted shrink-0" />
        <span className="text-xs text-blue-400 font-mono shrink-0">WHERE</span>
        <input
          type="text"
          value={filterInput}
          onChange={(e) => setFilterInput(e.target.value)}
          onBlur={commitChanges}
          onKeyDown={handleKeyDown}
          className="bg-transparent border-none outline-none text-xs text-secondary w-full placeholder:text-surface-tertiary font-mono"
          placeholder={`${placeholderColumn} > 5 AND status = 'active'`}
        />
      </div>
      <div className="flex items-center gap-2 flex-1 bg-base border border-default rounded px-2 py-1 focus-within:border-blue-500/50 transition-colors">
        <ArrowUpDown size={14} className="text-muted shrink-0" />
        <span className="text-xs text-blue-400 font-mono shrink-0">ORDER BY</span>
        <input
          type="text"
          value={sortInput}
          onChange={(e) => setSortInput(e.target.value)}
          onBlur={commitChanges}
          onKeyDown={handleKeyDown}
          className="bg-transparent border-none outline-none text-xs text-secondary w-full placeholder:text-surface-tertiary font-mono"
          placeholder={`${placeholderSort} DESC`}
        />
      </div>
      <div className="flex items-center gap-2 w-32 bg-base border border-default rounded px-2 py-1 focus-within:border-blue-500/50 transition-colors">
        <ListFilter size={14} className="text-muted shrink-0" />
        <span className="text-xs text-blue-400 font-mono shrink-0">LIMIT</span>
        <input
          type="number"
          value={limitInput}
          onChange={(e) => setLimitInput(e.target.value)}
          onBlur={commitChanges}
          onKeyDown={handleKeyDown}
          className="bg-transparent border-none outline-none text-xs text-secondary w-full placeholder:text-surface-tertiary font-mono"
          placeholder={String(defaultLimit)}
        />
      </div>
    </div>
  );
};

export const TableToolbar: React.FC<TableToolbarProps> = (props) => {
  // Use a stable key based on initial values to reset component when they change
  const stateKey = `${props.initialFilter}-${props.initialSort}-${props.initialLimit}`;
  return <TableToolbarInternal key={stateKey} {...props} />;
};
