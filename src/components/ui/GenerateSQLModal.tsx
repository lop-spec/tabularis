import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Loader2, Copy, Check, FileCode } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useDatabase } from '../../hooks/useDatabase';
import { SqlPreview } from './SqlPreview';
import { message } from '@tauri-apps/plugin-dialog';

interface TableColumn {
  name: string;
  data_type: string;
  is_pk: boolean;
  is_nullable: boolean;
  is_auto_increment: boolean;
  default_value: string | null;
}

interface ForeignKey {
  name: string;
  column_name: string;
  ref_table: string;
  ref_column: string;
}

interface Index {
  name: string;
  column_name: string;
  is_unique: boolean;
  is_primary: boolean;
}

interface GenerateSQLModalProps {
  isOpen: boolean;
  onClose: () => void;
  tableName: string;
}

export const GenerateSQLModal = ({ isOpen, onClose, tableName }: GenerateSQLModalProps) => {
  const { t } = useTranslation();
  const { activeConnectionId, activeDriver } = useDatabase();
  const [sql, setSql] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!isOpen || !activeConnectionId || !tableName) return;

    const generateSQL = async () => {
      setLoading(true);
      try {
        const [columns, foreignKeys, indexes] = await Promise.all([
          invoke<TableColumn[]>('get_columns', {
            connectionId: activeConnectionId,
            tableName
          }),
          invoke<ForeignKey[]>('get_foreign_keys', {
            connectionId: activeConnectionId,
            tableName
          }),
          invoke<Index[]>('get_indexes', { connectionId: activeConnectionId, tableName }),
        ]);

        const q = activeDriver === 'mysql' || activeDriver === 'mariadb' ? '`' : '"';
        const lines: string[] = [];
        lines.push(`CREATE TABLE ${q}${tableName}${q} (`);

        const columnDefs = columns.map(col => {
          let def = `  ${q}${col.name}${q} ${col.data_type}`;

          if (!col.is_nullable) {
            def += ' NOT NULL';
          }

          if (col.default_value !== null && col.default_value !== undefined) {
            def += ` DEFAULT ${col.default_value}`;
          }

          if (col.is_auto_increment) {
            if (activeDriver === 'mysql' || activeDriver === 'mariadb') {
              def += ' AUTO_INCREMENT';
            } else if (activeDriver === 'sqlite') {
              def = def.replace(new RegExp(`^\\s*${q}${col.name}${q}\\s*`), `  ${q}${col.name}${q} INTEGER PRIMARY KEY AUTOINCREMENT `);
            } else if (activeDriver === 'postgresql') {
              def = def.replace(new RegExp(`^\\s*${q}${col.name}${q}\\s*`), `  ${q}${col.name}${q} SERIAL `);
            }
          }

          return def;
        });

        const pkColumns = columns.filter(c => c.is_pk).map(c => `${q}${c.name}${q}`);
        if (pkColumns.length > 0 && activeDriver !== 'sqlite') {
          columnDefs.push(`  PRIMARY KEY (${pkColumns.join(', ')})`);
        }

        foreignKeys.forEach(fk => {
          const fkDef = `  CONSTRAINT ${q}${fk.name}${q} FOREIGN KEY (${q}${fk.column_name}${q}) REFERENCES ${q}${fk.ref_table}${q} (${q}${fk.ref_column}${q})`;
          columnDefs.push(fkDef);
        });

        lines.push(columnDefs.join(',\n'));
        lines.push(');');

        const uniqueIndexes = indexes.filter(idx => idx.is_unique && !idx.is_primary);
        if (uniqueIndexes.length > 0) {
          lines.push('');
          uniqueIndexes.forEach(idx => {
            lines.push(`CREATE UNIQUE INDEX ${q}${idx.name}${q} ON ${q}${tableName}${q} (${q}${idx.column_name}${q});`);
          });
        }

        const nonUniqueIndexes = indexes.filter(idx => !idx.is_unique && !idx.is_primary);
        if (nonUniqueIndexes.length > 0) {
          lines.push('');
          nonUniqueIndexes.forEach(idx => {
            lines.push(`CREATE INDEX ${q}${idx.name}${q} ON ${q}${tableName}${q} (${q}${idx.column_name}${q});`);
          });
        }

        setSql(lines.join('\n'));
      } catch (err) {
        console.error(err);
        await message(String(err), { title: t('common.error'), kind: 'error' });
      } finally {
        setLoading(false);
      }
    };

    void generateSQL();
  }, [isOpen, activeConnectionId, tableName, activeDriver, t]);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(sql);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-[100] backdrop-blur-sm">
      <div className="bg-elevated border border-strong rounded-xl shadow-2xl w-[900px] max-w-[90vw] h-[80vh] max-h-[800px] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-default bg-base">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-blue-900/30 rounded-lg">
              <FileCode size={20} className="text-blue-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-primary">{t('generateSQL.title', { table: tableName })}</h2>
            </div>
          </div>
          <button onClick={onClose} className="text-secondary hover:text-primary transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 p-6 overflow-hidden flex flex-col">
          {loading ? (
            <div className="text-center py-8 text-muted">
              <Loader2 size={24} className="animate-spin mx-auto mb-2" />
              <span>{t('generateSQL.loading')}</span>
            </div>
          ) : (
            <div className="flex-1 flex flex-col gap-4">
              <div className="flex-1 overflow-hidden rounded-lg border border-default">
                <SqlPreview
                  sql={sql}
                  height="100%"
                  showLineNumbers={true}
                  className="h-full"
                />
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        {!loading && (
          <div className="p-4 border-t border-default bg-base/50 flex justify-end gap-3">
            <button
              onClick={handleCopy}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm font-medium transition-colors flex items-center gap-2"
            >
              {copied ? <Check size={16} /> : <Copy size={16} />}
              {copied ? t('generateSQL.copied') : t('generateSQL.copy')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
