const RESULTS_PANEL_RATIO = 1 / 3;
const EDITOR_CHROME_HEIGHT_PX = 86;

export function getDefaultEditorHeight(): string {
  const editorRatioPercent = (1 - RESULTS_PANEL_RATIO) * 100;
  return `calc(${editorRatioPercent.toFixed(6)}% - ${EDITOR_CHROME_HEIGHT_PX}px)`;
}
