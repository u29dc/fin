/**
 * Table output using console-table-printer.
 * Provides consistent formatting across all commands.
 */

import { Table } from 'console-table-printer';

import { log, json as logJson } from './logger';

export type Column<T = Record<string, unknown>> = {
	/** Key in the row object */
	key: keyof T & string;
	/** Column header label */
	label: string;
	/** Minimum width */
	minWidth?: number;
	/** Maximum width (values are truncated when longer) */
	maxWidth?: number;
	/** Text alignment */
	align?: 'left' | 'right';
	/** Custom formatter for cell values */
	format?: (value: unknown) => string;
};

function truncate(value: string, width: number): string {
	if (value.length <= width) return value;
	if (width <= 3) return value.slice(0, width);
	return `${value.slice(0, width - 3)}...`;
}

/**
 * Render data as a table using console-table-printer.
 */
export function table<T extends Record<string, unknown>>(rows: T[], columns: Column<T>[]): string {
	if (rows.length === 0) {
		return 'No results.';
	}

	const tableColumns = columns.map((col) => {
		const colConfig: { name: string; title: string; alignment: 'left' | 'right'; minLen?: number; maxLen?: number } = {
			name: col.key,
			title: col.label.toUpperCase(),
			alignment: col.align ?? 'left',
		};
		if (col.minWidth !== undefined) colConfig.minLen = col.minWidth;
		if (col.maxWidth !== undefined) colConfig.maxLen = col.maxWidth;
		return colConfig;
	});

	const t = new Table({
		style: {
			headerTop: { left: '', mid: '', right: '', other: '' },
			headerBottom: { left: '', mid: '', right: '', other: '' },
			tableBottom: { left: '', mid: '', right: '', other: '' },
			vertical: '',
			rowSeparator: { left: '', mid: '', right: '', other: '' },
		},
		columns: tableColumns,
	});

	for (const row of rows) {
		const formattedRow: Record<string, string> = {};
		for (const col of columns) {
			const val = row[col.key];
			let formatted = col.format ? col.format(val) : String(val ?? '');
			if (col.maxWidth) {
				formatted = truncate(formatted, col.maxWidth);
			}
			formattedRow[col.key] = formatted;
		}
		t.addRow(formattedRow);
	}

	return t.render();
}

/**
 * Render data as TSV (tab-separated values).
 * Useful for piping to other tools.
 */
export function toTsv<T extends Record<string, unknown>>(rows: T[], columns: Column<T>[]): string {
	if (rows.length === 0) {
		return '';
	}

	// Header row
	const header = columns.map((col) => col.label).join('\t');

	// Data rows
	const dataRows = rows.map((row) =>
		columns
			.map((col) => {
				const val = row[col.key];
				const formatted = col.format ? col.format(val) : String(val ?? '');
				// Escape tabs and newlines in values
				return formatted.replace(/[\t\n]/g, ' ');
			})
			.join('\t'),
	);

	return [header, ...dataRows].join('\n');
}

/**
 * Render a summary line (displayed after table).
 */
export function summary(text: string): string {
	return `\n${text}`;
}

/**
 * Render output based on format option.
 */
export function renderOutput<T extends Record<string, unknown>>(rows: T[], columns: Column<T>[], format: 'table' | 'json' | 'tsv', summaryText?: string): void {
	switch (format) {
		case 'json':
			logJson(rows);
			break;
		case 'tsv':
			log(toTsv(rows, columns));
			break;
		default:
			log(table(rows, columns));
			if (summaryText) {
				log(summary(summaryText));
			}
	}
}

/**
 * Parse format option from string.
 */
export function parseFormat(value: string | undefined): 'table' | 'json' | 'tsv' {
	if (value === 'json' || value === 'tsv') {
		return value;
	}
	return 'table';
}
