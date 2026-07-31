import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface TableNameInputProps {
  value: string;
  onChange: (value: string) => void;
  tableExists: boolean;
}

export function TableNameInput({
  value,
  onChange,
  tableExists,
}: TableNameInputProps) {
  const { t } = useTranslation();

  return (
    <div>
      <label className="block text-xs uppercase font-bold text-muted mb-1">
        {t('clipboardImport.tableName')}
      </label>
      <div className="flex gap-2">
        <div className="relative flex-1">
          <input
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={t('clipboardImport.tableNamePlaceholder')}
            className={`w-full bg-base border rounded-lg px-3 py-2 text-primary focus:outline-none transition-all font-mono text-sm ${
              tableExists ? 'border-yellow-500/60 focus:border-yellow-500' : 'border-strong focus:border-blue-500'
            }`}
            autoFocus
          />
          {tableExists && (
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1 text-yellow-400 text-[10px]">
              <AlertTriangle size={12} />
              <span>{t('clipboardImport.tableExists')}</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
