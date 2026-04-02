import React, { useRef, useCallback, useEffect } from "react";
import MonacoEditor, { type OnMount, type BeforeMount } from "@monaco-editor/react";
import type * as Monaco from "monaco-editor";
import { useTheme } from "../../hooks/useTheme";
import { loadMonacoTheme } from "../../themes/themeUtils";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { useSettings } from "../../hooks/useSettings";
import { getFontCSS } from "../../utils/settings";

interface SqlEditorWrapperProps {
  initialValue: string;
  onChange: (value: string) => void;
  onRun: () => void;
  onMount?: OnMount;
  height?: string | number;
  options?: React.ComponentProps<typeof MonacoEditor>['options'];
  editorKey?: string;
}

// Internal component that resets when key changes
const SqlEditorInternal = ({
  initialValue,
  onChange,
  onRun,
  onMount,
  height = "100%",
  options
}: SqlEditorWrapperProps & { editorKey: string }) => {
  const updateTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const monacoRef = useRef<typeof Monaco | null>(null);
  const { currentTheme, allThemes } = useTheme();
  const { settings } = useSettings();

  const editorTheme = settings.editorTheme
    ? (allThemes.find((t) => t.id === settings.editorTheme) ?? currentTheme)
    : currentTheme;

  // Sync editor value only when initialValue changes externally (e.g., tab switch)
  useEffect(() => {
    if (editorRef.current && initialValue !== editorRef.current.getValue()) {
      editorRef.current.setValue(initialValue);
    }
  }, [initialValue]);

  // Update Monaco theme when theme changes
  useEffect(() => {
    if (editorRef.current && monacoRef.current) {
      loadMonacoTheme(editorTheme, monacoRef.current);
    }
  }, [editorTheme]);

    const handleChange = useCallback(
      (val: string | undefined) => {
        const newValue = val || "";

        if (updateTimeoutRef.current) {
          clearTimeout(updateTimeoutRef.current);
        }

        updateTimeoutRef.current = setTimeout(() => {
          onChange(newValue);
        }, 300);
      },
      [onChange]
    );

    const handleBeforeMount: BeforeMount = (monaco) => {
      // Load Monaco theme before editor is created
      loadMonacoTheme(editorTheme, monaco);
    };

    const tauriPaste = async (ed: Monaco.editor.ICodeEditor) => {
      try {
        const text = await readText();
        const selection = ed.getSelection();
        if (selection && text) {
          ed.executeEdits('paste', [{
            range: selection,
            text: text,
            forceMoveMarkers: true
          }]);
          ed.pushUndoStop();
        }
      } catch (err) {
        console.error('Failed to read clipboard:', err);
      }
    };

    const handleEditorMount: OnMount = (editor, monaco) => {
      editorRef.current = editor;
      monacoRef.current = monaco;

      // Register custom Paste action using Tauri clipboard API
      editor.addAction({
        id: 'tauri.clipboardPaste',
        label: 'Paste',
        contextMenuGroupId: '9_cutcopypaste',
        contextMenuOrder: 2,
        keybindings: [monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyV],
        run: tauriPaste,
      });

      // Remove the built-in Paste from context menu (doesn't work in Tauri)
      const contextMenuContrib = editor.getContribution('editor.contrib.contextmenu');
      if (contextMenuContrib) {
        const orig = (contextMenuContrib as any)._getMenuActions;
        if (typeof orig === 'function') {
          (contextMenuContrib as any)._getMenuActions = function (...args: unknown[]) {
            const actions: { id?: string }[] = orig.apply(this, args);
            return actions.filter((a) => a.id !== 'editor.action.clipboardPasteAction');
          };
        }
      }

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
        theme={editorTheme.id}
        defaultValue={initialValue}
        onChange={handleChange}
        beforeMount={handleBeforeMount}
        onMount={handleEditorMount}
        options={{
          minimap: { enabled: false },
          fontSize: settings.editorFontSize ?? 14,
          fontFamily: getFontCSS(settings.editorFontFamily ?? "JetBrains Mono"),
          lineHeight: settings.editorLineHeight ?? 1.5,
          tabSize: settings.editorTabSize ?? 2,
          wordWrap: (settings.editorWordWrap ?? true) ? 'on' : 'off',
          lineNumbers: (settings.editorShowLineNumbers ?? true) ? 'on' : 'off',
          padding: { top: 16, bottom: 32 },
          scrollBeyondLastLine: false,
          overviewRulerLanes: 0,
          hideCursorInOverviewRuler: true,
          overviewRulerBorder: false,
          scrollbar: {
            vertical: 'auto',
            horizontal: 'auto',
            verticalScrollbarSize: 8,
            horizontalScrollbarSize: 8,
          },
          automaticLayout: true,
          ...options
        }}
      />
    );
};

export const SqlEditorWrapper = React.memo((props: SqlEditorWrapperProps) => {
  // Use editorKey to control when component remounts (only on tab switch)
  return <SqlEditorInternal key={props.editorKey || "default"} editorKey={props.editorKey || "default"} {...props} />;
});
