import { SqlEditorWrapper } from "../ui/SqlEditorWrapper";
import { useSettings } from "../../hooks/useSettings";
import { NotebookAiButtons } from "./NotebookAiButtons";

interface SqlCellEditorProps {
  cellId: string;
  content: string;
  onContentChange: (content: string) => void;
  onRun: () => void;
}

export function SqlCellEditor({
  cellId,
  content,
  onContentChange,
  onRun,
}: SqlCellEditorProps) {
  const { settings } = useSettings();

  return (
    <div className="h-[150px] relative">
      <SqlEditorWrapper
        height="100%"
        initialValue={content}
        onChange={onContentChange}
        onRun={onRun}
        editorKey={`notebook-${cellId}`}
        options={{
          padding: { top: 8, bottom: 8 },
          lineNumbers: "off",
          scrollbar: { alwaysConsumeMouseWheel: false },
        }}
      />
      {settings.aiEnabled && (
        <NotebookAiButtons content={content} onInsert={onContentChange} />
      )}
    </div>
  );
}
