/**
 * Elegant TUI table and output formatting.
 * Clean minimal design - no box-drawing characters.
 */

export type Column<T = Record<string, unknown>> = {
	/** Key in the row object */
	key: keyof T & string;
	/** Column header label */
	label: string;
	/** Minimum width (actual width is max of label, values, minWidth) */
	minWidth?: number;
	/** Maximum width (values are truncated when longer) */
	maxWidth?: number;
	/** Text alignment */
	align?: 'left' | 'right';
	/** Custom formatter for cell values */
	format?: (value: unknown) => string;
};

/**
 * Render data as an elegant minimal table.
 *
 * Style:
 * - Header in uppercase, followed by blank line separator
 * - Right-aligned numbers, left-aligned text
 * - Consistent column widths
 * - Clean spacing (2 spaces between columns)
 */
export function table<T extends Record<string, unknown>>(rows: T[], columns: Column<T>[]): string {
	if (rows.length === 0) {
		return 'No results.';
	}

	const gap = '  ';

	// Calculate column widths
	const widths = columns.map((col) => {
		const headerWidth = col.label.length;
		const valueWidths = rows.map((row) => {
			const val = row[col.key];
			const formatted = col.format ? col.format(val) : String(val ?? '');
			return formatted.length;
		});
		let width = Math.max(headerWidth, ...valueWidths, col.minWidth ?? 0);
		if (col.maxWidth && width > col.maxWidth) {
			width = col.maxWidth;
		}
		return width;
	});

	// Build header
	const header = columns
		.map((col, i) => {
			const width = widths[i] ?? 0;
			const label = truncate(col.label.toUpperCase(), width);
			return label.padEnd(width);
		})
		.join(gap);

	// Build data rows
	const dataRows = rows.map((row) =>
		columns
			.map((col, i) => {
				const val = row[col.key];
				let formatted = col.format ? col.format(val) : String(val ?? '');
				const width = widths[i] ?? 0;
				if (col.maxWidth) {
					formatted = truncate(formatted, width);
				}
				return col.align === 'right' ? formatted.padStart(width) : formatted.padEnd(width);
			})
			.join(gap),
	);

	// Combine with blank line separator after header
	return [header, '', ...dataRows].join('\n');
}

function truncate(value: string, width: number): string {
	if (value.length <= width) return value;
	if (width <= 3) return value.slice(0, width);
	return `${value.slice(0, width - 3)}...`;
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
			console.log(JSON.stringify(rows, null, 2));
			break;
		case 'tsv':
			console.log(toTsv(rows, columns));
			break;
		default:
			console.log(table(rows, columns));
			if (summaryText) {
				console.log(summary(summaryText));
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
