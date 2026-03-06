export type ColorScheme = 'light' | 'dark';

export type TreemapDataItem = {
	name: string;
	value: number;
	children?: TreemapDataItem[];
};

export type SankeyNode = {
	name: string;
	itemStyle?: {
		color: string;
	};
};

export type SankeyLink = {
	source: string;
	target: string;
	value: number;
};

export type LineChartDataPoint = [string | number, number];

export type LineSeriesDefinition = {
	key: string;
	data: LineChartDataPoint[];
	color: string;
	lineStyle?: 'solid' | 'dashed' | 'dotted';
	lineWidth?: number;
	smooth?: boolean;
	showSymbol?: boolean;
	areaStyle?: {
		color?: string | object;
		opacity?: number;
	};
};

export type MarkLineItem = {
	yAxis: number;
	name?: string;
	label?: {
		formatter?: string;
		position?: 'start' | 'middle' | 'end';
	};
	lineStyle?: {
		color?: string;
		type?: 'solid' | 'dashed' | 'dotted';
		width?: number;
	};
};

export type LineChartOptions = {
	colorScheme?: ColorScheme;
	height?: number;
	compact?: boolean;
	showTooltip?: boolean;
	formatTooltip?: (params: unknown) => string;
	formatYAxis?: (value: number) => string;
	markLines?: MarkLineItem[];
	xAxisType?: 'time' | 'category';
};
