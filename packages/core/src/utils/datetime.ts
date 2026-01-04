function pad2(value: number): string {
	return value.toString().padStart(2, '0');
}

export function toIsoLocalDateTime(datePart: string, timePart: string): string {
	const [dayStr, monthStr, yearStr] = datePart.trim().split(/[/-]/);
	if (!dayStr || !monthStr || !yearStr) {
		throw new Error(`Invalid date: ${datePart}`);
	}

	const [hourStr, minuteStr, secondStr = '00'] = timePart.trim().split(':');
	if (!hourStr || !minuteStr) {
		throw new Error(`Invalid time: ${timePart}`);
	}

	const year = Number(yearStr);
	const month = Number(monthStr);
	const day = Number(dayStr);
	const hour = Number(hourStr);
	const minute = Number(minuteStr);
	const second = Number(secondStr);

	return `${year}-${pad2(month)}-${pad2(day)}T${pad2(hour)}:${pad2(minute)}:${pad2(second)}`;
}

export function parseWiseDateTime(dateTimePart: string, fallbackDatePart?: string): string {
	const source = dateTimePart.trim() || fallbackDatePart?.trim() || '';
	if (source.length === 0) {
		throw new Error('Missing Wise date');
	}

	const parts = source.split(' ');
	const datePart = parts[0];
	const timePart = parts[1] ?? '00:00:00';
	if (!datePart) {
		throw new Error(`Invalid Wise date: ${source}`);
	}

	const [dayStr, monthStr, yearStr] = datePart.split('-');
	if (!dayStr || !monthStr || !yearStr) {
		throw new Error(`Invalid Wise date: ${source}`);
	}

	const [hourStr, minuteStr, secondStr = '00'] = timePart.split(':');
	const secondOnly = secondStr.split('.')[0] ?? '00';

	return toIsoLocalDateTime(`${dayStr}-${monthStr}-${yearStr}`, `${hourStr ?? '00'}:${minuteStr ?? '00'}:${secondOnly}`);
}
