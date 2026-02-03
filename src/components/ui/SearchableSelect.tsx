import { useState, useRef, useEffect } from "react";
import { Search, ChevronDown, X } from "lucide-react";
import clsx from "clsx";

interface SearchableSelectProps {
  value: string | null;
  options: string[];
  onChange: (value: string) => void;
  placeholder?: string;
  searchPlaceholder?: string;
  noResultsLabel?: string;
  disabled?: boolean;
  className?: string;
  hasError?: boolean;
}

export const SearchableSelect = ({
  value,
  options,
  onChange,
  placeholder = "Select option",
  searchPlaceholder = "Search...",
  noResultsLabel = "No results found",
  disabled = false,
  className,
  hasError = false,
}: SearchableSelectProps) => {
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Close when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  // Focus search input when opening
  useEffect(() => {
    if (isOpen && searchInputRef.current) {
        // Small timeout to allow render
        setTimeout(() => searchInputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  const filteredOptions = options.filter((option) =>
    option.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSelect = (option: string) => {
    onChange(option);
    setIsOpen(false);
    setSearchQuery("");
  };

  return (
    <div className={clsx("relative", className)} ref={containerRef}>
      <button
        type="button"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        className={clsx(
          "w-full bg-base border rounded px-3 py-2 text-primary flex items-center justify-between transition-colors",
          disabled
            ? "opacity-50 cursor-not-allowed border-default"
            : hasError 
                ? "border-red-500 hover:border-red-400" 
                : "border-strong hover:border-blue-500 cursor-pointer",
          isOpen && !disabled && !hasError ? "border-blue-500 ring-1 ring-blue-500" : ""
        )}
      >
        <span className={clsx("truncate", !value && "text-muted", hasError && "text-red-400")}>
          {value || placeholder}
        </span>
        <ChevronDown size={16} className={clsx("shrink-0 ml-2", hasError ? "text-red-400" : "text-secondary")} />
      </button>

      {isOpen && !disabled && (
        <div className="absolute z-50 top-full left-0 right-0 mt-1 bg-elevated border border-strong rounded-lg shadow-xl max-h-60 flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-100">
          <div className="p-2 border-b border-default bg-elevated">
            <div className="flex items-center gap-2 bg-base border border-strong rounded px-2 py-1.5 focus-within:border-blue-500 transition-colors">
              <Search size={14} className="text-muted shrink-0" />
              <input
                ref={searchInputRef}
                type="text"
                placeholder={searchPlaceholder}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full bg-transparent border-none text-sm text-primary focus:outline-none placeholder:text-muted"
                onKeyDown={(e) => {
                    if (e.key === 'Enter' && filteredOptions.length > 0) {
                        handleSelect(filteredOptions[0]);
                    }
                    if (e.key === 'Escape') {
                        setIsOpen(false);
                    }
                }}
              />
              {searchQuery && (
                <button 
                    onClick={() => setSearchQuery("")}
                    className="text-muted hover:text-primary"
                >
                    <X size={14} />
                </button>
              )}
            </div>
          </div>
          
          <div className="overflow-y-auto flex-1 p-1 scrollbar-thin scrollbar-thumb-surface-tertiary scrollbar-track-transparent">
            {filteredOptions.length === 0 ? (
              <div className="p-3 text-sm text-muted text-center italic">
                {noResultsLabel}
              </div>
            ) : (
              filteredOptions.map((option) => (
                <button
                  key={option}
                  onClick={() => handleSelect(option)}
                  className={clsx(
                    "w-full text-left px-3 py-2 text-sm rounded transition-colors truncate",
                    value === option
                      ? "bg-blue-600/10 text-blue-400 font-medium"
                      : "text-primary hover:bg-surface-secondary"
                  )}
                  title={option}
                >
                  {option}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
};
