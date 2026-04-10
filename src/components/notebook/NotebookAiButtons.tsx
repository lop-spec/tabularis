import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Sparkles, BookOpen, Network } from "lucide-react";
import { AiQueryModal } from "../modals/AiQueryModal";
import { AiExplainModal } from "../modals/AiExplainModal";
import { VisualExplainModal } from "../modals/VisualExplainModal";

interface NotebookAiButtonsProps {
  content: string;
  onInsert: (sql: string) => void;
  connectionId: string;
  schema?: string;
}

export function NotebookAiButtons({
  content,
  onInsert,
  connectionId,
  schema,
}: NotebookAiButtonsProps) {
  const { t } = useTranslation();
  const [isGenerateOpen, setIsGenerateOpen] = useState(false);
  const [isExplainOpen, setIsExplainOpen] = useState(false);
  const [isVisualExplainOpen, setIsVisualExplainOpen] = useState(false);

  return (
    <>
      <div className="absolute bottom-1 right-2 z-10 flex items-center gap-1">
        <button
          type="button"
          onClick={() => setIsVisualExplainOpen(true)}
          disabled={!content.trim() || !connectionId}
          className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] text-muted hover:text-green-300 bg-elevated/80 hover:bg-green-900/40 border border-default hover:border-green-500/40 transition-all disabled:opacity-30 disabled:pointer-events-none backdrop-blur-sm"
          title={t("editor.visualExplain.title")}
        >
          <Network size={10} />
          {t("editor.visualExplain.buttonShort")}
        </button>
        <button
          type="button"
          onClick={() => setIsGenerateOpen(true)}
          className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] text-muted hover:text-purple-300 bg-elevated/80 hover:bg-purple-900/40 border border-default hover:border-purple-500/40 transition-all backdrop-blur-sm"
          title={t("editor.notebook.aiGenerate")}
        >
          <Sparkles size={10} />
          AI
        </button>
        <button
          type="button"
          onClick={() => setIsExplainOpen(true)}
          disabled={!content.trim()}
          className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] text-muted hover:text-blue-300 bg-elevated/80 hover:bg-blue-900/40 border border-default hover:border-blue-500/40 transition-all disabled:opacity-30 disabled:pointer-events-none backdrop-blur-sm"
          title={t("editor.notebook.aiExplain")}
        >
          <BookOpen size={10} />
          Explain
        </button>
      </div>

      <AiQueryModal
        isOpen={isGenerateOpen}
        onClose={() => setIsGenerateOpen(false)}
        onInsert={(sql) => {
          onInsert(sql);
          setIsGenerateOpen(false);
        }}
      />
      <AiExplainModal
        isOpen={isExplainOpen}
        onClose={() => setIsExplainOpen(false)}
        query={content}
      />
      <VisualExplainModal
        isOpen={isVisualExplainOpen}
        onClose={() => setIsVisualExplainOpen(false)}
        query={content}
        connectionId={connectionId}
        schema={schema}
      />
    </>
  );
}
