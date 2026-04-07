import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Play } from 'lucide-react';
import { Modal } from '../ui/Modal';

interface QuerySelectionModalProps {
  isOpen: boolean;
  queries: string[];
  onSelect: (query: string) => void;
  onClose: () => void;
}

const QuerySelectionContent = ({ queries, onSelect, onClose }: Omit<QuerySelectionModalProps, 'isOpen'>) => {
  const { t } = useTranslation();
  const [focusedIndex, setFocusedIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    itemRefs.current[focusedIndex]?.scrollIntoView({ block: 'nearest' });
  }, [focusedIndex]);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setFocusedIndex(prev => Math.min(prev + 1, queries.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setFocusedIndex(prev => Math.max(prev - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      onSelect(queries[focusedIndex]);
    } else {
      const num = parseInt(e.key, 10);
      if (num >= 1 && num <= 9 && num <= queries.length) {
        e.preventDefault();
        onSelect(queries[num - 1]);
      }
    }
  }, [queries, focusedIndex, onSelect]);

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  return (
    <div className="bg-elevated border border-default rounded-xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-default">
        <h3 className="text-lg font-semibold text-white">{t('editor.querySelection.title')}</h3>
        <button
          onClick={onClose}
          className="text-secondary hover:text-white transition-colors"
        >
          <X size={20} />
        </button>
      </div>

      {/* Content */}
      <div ref={listRef} className="flex-1 overflow-y-auto p-4 space-y-2">
        {queries.map((q, i) => (
          <div
            key={i}
            ref={el => { itemRefs.current[i] = el; }}
            onClick={() => onSelect(q)}
            onMouseEnter={() => setFocusedIndex(i)}
            className={`p-3 bg-surface-secondary/50 hover:bg-surface-secondary border rounded-lg cursor-pointer group transition-all ${
              focusedIndex === i ? 'border-blue-500 bg-surface-secondary' : 'border-strong hover:border-blue-500'
            }`}
          >
            <div className="flex items-start gap-3">
              <div className="flex items-center gap-2 shrink-0 mt-1">
                <span className={`w-6 h-6 flex items-center justify-center rounded text-xs font-bold ${
                  focusedIndex === i ? 'bg-blue-600 text-white' : 'bg-surface-secondary text-muted'
                } transition-colors`}>
                  {i + 1}
                </span>
                <div className={`p-1 rounded transition-colors ${
                  focusedIndex === i ? 'bg-blue-600 text-white' : 'bg-blue-900/30 text-blue-400 group-hover:bg-blue-600 group-hover:text-white'
                }`}>
                  <Play size={14} fill="currentColor" />
                </div>
              </div>
              <pre className="text-sm font-mono text-secondary overflow-hidden whitespace-pre-wrap break-all line-clamp-3">
                {q}
              </pre>
            </div>
          </div>
        ))}
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-default bg-elevated/50 text-xs text-muted flex justify-between">
          <span>{t('editor.querySelection.queriesFound', { count: queries.length })}</span>
          <span>{t('editor.querySelection.numberHint')} · {t('editor.querySelection.escToCancel')}</span>
      </div>
    </div>
  );
};

export const QuerySelectionModal = ({ isOpen, queries, onSelect, onClose }: QuerySelectionModalProps) => {
  return (
    <Modal isOpen={isOpen} onClose={onClose} overlayClassName="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      {isOpen && (
        <QuerySelectionContent
          key={queries.join('\n')}
          queries={queries}
          onSelect={onSelect}
          onClose={onClose}
        />
      )}
    </Modal>
  );
};
