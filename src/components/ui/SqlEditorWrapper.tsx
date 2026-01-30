import React, { useState, useRef, useCallback } from "react";
import MonacoEditor, { type OnMount } from "@monaco-editor/react";

interface SqlEditorWrapperProps {
  initialValue: string;
  onChange: (value: string) => void;
  onRun: () => void;
  onMount?: OnMount;
  height?: string | number;
  options?: React.ComponentProps<typeof MonacoEditor>['options'];
}

// Internal component that resets when key changes
const SqlEditorInternal: React.FC<SqlEditorWrapperProps & { editorKey: string }> = ({
  initialValue,
  onChange,
  onRun,
  onMount,
  height = "100%",
  options
}) => {
  const [localValue, setLocalValue] = useState(initialValue);
  const updateTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);

    const handleChange = useCallback(
      (val: string | undefined) => {
        const newValue = val || "";
        setLocalValue(newValue);

        if (updateTimeoutRef.current) {
          clearTimeout(updateTimeoutRef.current);
        }

        updateTimeoutRef.current = setTimeout(() => {
          onChange(newValue);
        }, 300);
      },
      [onChange]
    );

    const handleEditorMount: OnMount = (editor, monaco) => {
      editorRef.current = editor;
      
      // Bind Ctrl+Enter to Run
      editor.addCommand(
        monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter,
        () => {
            onRun();
        }
      );

      if (onMount) onMount(editor, monaco);
    };

    return (
      <MonacoEditor
        height={height}
        defaultLanguage="sql"
        theme="vs-dark"
        value={localValue}
        onChange={handleChange}
        onMount={handleEditorMount}
        options={{
          minimap: { enabled: false },
          fontSize: 14,
          padding: { top: 16 },
          scrollBeyondLastLine: false,
          automaticLayout: true,
          ...options
        }}
      />
    );
};

export const SqlEditorWrapper: React.FC<SqlEditorWrapperProps> = React.memo((props) => {
  // Use initialValue as key to reset component state when it changes
  return <SqlEditorInternal key={props.initialValue} editorKey={props.initialValue} {...props} />;
});
