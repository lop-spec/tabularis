/**
 * Clipboard utility functions for DataGrid copy operations
 */

import { formatCellValue } from './dataGrid';

/**
 * Converts a single row to TSV (Tab-Separated Values) format
 * @param row - Array of cell values
 * @param nullLabel - Label to use for null values
 * @returns TSV-formatted string
 */
export function rowToTSV(row: unknown[], nullLabel: string = "null"): string {
  return row
    .map((cell) => formatCellValue(cell, nullLabel))
    .join("\t");
}

/**
 * Converts multiple rows to TSV format with newline separators
 * @param rows - Array of row arrays
 * @param nullLabel - Label to use for null values
 * @returns TSV-formatted string with newlines
 */
export function rowsToTSV(rows: unknown[][], nullLabel: string = "null"): string {
  return rows
    .map((row) => rowToTSV(row, nullLabel))
    .join("\n");
}

/**
 * Gets selected rows from data based on selected indices
 * @param data - All data rows
 * @param selectedIndices - Set of selected row indices
 * @returns Array of selected rows in sorted order
 */
export function getSelectedRows(
  data: unknown[][],
  selectedIndices: Set<number>
): unknown[][] {
  const sortedIndices = Array.from(selectedIndices).sort((a, b) => a - b);
  return sortedIndices.map((idx) => data[idx]);
}

/**
 * Copies text to clipboard with error handling
 * @param text - Text to copy
 * @param onError - Optional error handler
 * @returns Promise that resolves when copy is complete
 */
export async function copyTextToClipboard(
  text: string,
  onError?: (error: unknown) => void
): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch (e) {
    console.error("Copy failed:", e);
    if (onError) {
      onError(e);
    } else {
      throw e;
    }
  }
}
