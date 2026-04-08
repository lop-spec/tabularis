import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Play, Check } from 'lucide-react';
import { Modal } from '../ui/Modal';

interface QuerySelectionModalProps {
  isOpen: boolean;
  queries: string[];
  onSelect: (query: string) => void;
  onRunAll: (queries: string[]) => void;
  onRunSelected: (queries: string[]) => void;
  onClose: () => void;
}

const QuerySelectionContent = ({ queries, onSelect, onRunAll, onRunSelected, onClose }: Omit<QuerySelectionModalProps, 'isOpen'>) => {
  const { t } = useTranslation();
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [selectedIndices, setSelectedIndices] = useState<Set<number>>(new Set());
  const listRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    itemRefs.current[focusedIndex]?.scrollIntoView({ block: 'nearest' });
  }, [focusedIndex]);

  const toggleSelection = useCallback((index: number, e?: React.MouseEvent) => {
    e?.stopPropagation();
    setSelectedIndices(prev => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  }, []);

  const toggleAll = useCallback(() => {
    setSelectedIndices(prev =>
      prev.size === queries.length ? new Set() : new Set(queries.map((_, i) => i))
    );
  }, [queries]);

  const handleRunSelected = useCallback(() => {
    if (selectedIndices.size === 0) return;
    const selected = queries.filter((_, i) => selectedIndices.has(i));
    onRunSelected(selected);
  }, [queries, selectedIndices, onRunSelected]);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setFocusedIndex(prev => Math.min(prev + 1, queries.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setFocusedIndex(prev => Math.max(prev - 1, 0));
    } else if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
      e.preventDefault();
      onSelect(queries[focusedIndex]);
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      onRunAll(queries);
    } else if (e.key === 'Enter' && e.shiftKey) {
      e.preventDefault();
      handleRunSelected();
    } else if (e.key === ' ') {
      e.preventDefault();
      toggleSelection(focusedIndex);
    } else {
      const num = parseInt(e.key, 10);
      if (num >= 1 && num <= 9 && num <= queries.length) {
        e.preventDefault();
        onSelect(queries[num - 1]);
      }
    }
  }, [queries, focusedIndex, onSelect, onRunAll, handleRunSelected, toggleSelection]);

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  const allSelected = selectedIndices.size === queries.length;

  return (
    <div className="bg-elevated border border-default rounded-xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-default">
        <h3 className="text-lg font-semibold text-white">{t('editor.querySelection.title')}</h3>
        <div className="flex items-center gap-2">
          <button
            onClick={toggleAll}
            className="text-xs text-secondary hover:text-white transition-colors px-2 py-1 rounded hover:bg-surface-secondary"
          >
            {allSelected ? t('editor.querySelection.deselectAll') : t('editor.querySelection.selectAll')}
          </button>
          <button
            onClick={onClose}
            className="text-secondary hover:text-white transition-colors"
          >
            <X size={20} />
          </button>
        </div>
      </div>

      {/* Content */}
      <div ref={listRef} className="flex-1 overflow-y-auto p-4 space-y-2">
        {queries.map((q, i) => (
          <div
            key={i}
            ref={el => { itemRefs.current[i] = el; }}
            onMouseEnter={() => setFocusedIndex(i)}
            className={`p-3 bg-surface-secondary/50 hover:bg-surface-secondary border rounded-lg cursor-pointer group transition-all ${
              focusedIndex === i ? 'border-blue-500 bg-surface-secondary' : 'border-strong hover:border-blue-500'
            }`}
          >
            <div className="flex items-start gap-3">
              {/* Checkbox */}
              <button
                onClick={(e) => toggleSelection(i, e)}
                className={`w-5 h-5 mt-1 shrink-0 rounded border flex items-center justify-center transition-colors ${
                  selectedIndices.has(i)
                    ? 'bg-blue-600 border-blue-600 text-white'
                    : 'border-strong hover:border-blue-500 text-transparent'
                }`}
              >
                <Check size={12} />
              </button>
              <div className="flex items-center gap-2 shrink-0 mt-1">
                <span className={`w-6 h-6 flex items-center justify-center rounded text-xs font-bold ${
                  focusedIndex === i ? 'bg-blue-600 text-white' : 'bg-surface-secondary text-muted'
                } transition-colors`}>
                  {i + 1}
                </span>
                <button
                  onClick={(e) => { e.stopPropagation(); onSelect(q); }}
                  className={`p-1 rounded transition-colors ${
                    focusedIndex === i ? 'bg-blue-600 text-white' : 'bg-blue-900/30 text-blue-400 group-hover:bg-blue-600 group-hover:text-white'
                  }`}
                  title="Run this query"
                >
                  <Play size={14} fill="currentColor" />
                </button>
              </div>
              <pre
                className="text-sm font-mono text-secondary overflow-hidden whitespace-pre-wrap break-all line-clamp-3 flex-1"
                onClick={() => onSelect(q)}
              >
                {q}
              </pre>
            </div>
          </div>
        ))}
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-default bg-elevated/50 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <button
            onClick={() => onRunAll(queries)}
            className="px-3 py-1.5 bg-green-600 hover:bg-green-500 text-white text-xs font-medium rounded transition-colors"
          >
            {t('editor.querySelection.runAll')}
          </button>
          <button
            onClick={handleRunSelected}
            disabled={selectedIndices.size === 0}
            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium rounded transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t('editor.querySelection.runSelected', { count: selectedIndices.size })}
          </button>
        </div>
        <span className="text-xs text-muted">
          {t('editor.querySelection.queriesFound', { count: queries.length })} · {t('editor.querySelection.numberHint')} · {t('editor.querySelection.escToCancel')}
        </span>
      </div>
    </div>
  );
};

export const QuerySelectionModal = ({ isOpen, queries, onSelect, onRunAll, onRunSelected, onClose }: QuerySelectionModalProps) => {
  return (
    <Modal isOpen={isOpen} onClose={onClose} overlayClassName="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      {isOpen && (
        <QuerySelectionContent
          key={queries.join('\n')}
          queries={queries}
          onSelect={onSelect}
          onRunAll={onRunAll}
          onRunSelected={onRunSelected}
          onClose={onClose}
        />
      )}
    </Modal>
  );
};
