import { LineChart, SankeyChart, TreemapChart } from 'echarts/charts';
import { DataZoomComponent, GridComponent, LegendComponent, MarkLineComponent, TitleComponent, TooltipComponent, VisualMapComponent } from 'echarts/components';
import * as echarts from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';

// Register required components
echarts.use([LineChart, TreemapChart, SankeyChart, CanvasRenderer, GridComponent, TooltipComponent, TitleComponent, LegendComponent, DataZoomComponent, MarkLineComponent, VisualMapComponent]);

export { echarts };

export type ColorScheme = 'light' | 'dark';

export const ECHARTS_COLORS = {
	light: {
		background: 'transparent',
		text: '#374151',
		textMuted: '#6b7280',
		border: 'rgba(0, 0, 0, 0.08)',
		tooltip: {
			background: '#ffffff',
			border: '#e5e7eb',
			text: '#374151',
		},
	},
	dark: {
		background: 'transparent',
		text: '#e6e6e8',
		textMuted: '#9aa0a6',
		border: 'rgba(230, 230, 232, 0.08)',
		tooltip: {
			background: '#1b1e22',
			border: '#2d3139',
			text: '#e6e6e8',
		},
	},
} as const;

export const TREEMAP_PALETTE = {
	light: ['#e2e8f0', '#d8dfe8', '#ced6e0', '#c4ccd6', '#bac3cd'], // very light grays
	dark: ['#2d3139', '#282d34', '#23282f', '#1e2329', '#1a1e24'], // near-black grays
};

export const SANKEY_PALETTE = {
	light: {
		income: '#0d9488', // teal-600 (color-blind safe)
		asset: '#94a3b8', // slate-400
		expense: '#94a3b8', // slate-400
	},
	dark: {
		income: '#2dd4bf', // teal-400 (color-blind safe)
		asset: '#64748b', // slate-500
		expense: '#64748b', // slate-500
	},
};

export const DEFAULT_FONT_FAMILY = "'JetBrains Mono', 'SFMono-Regular', Menlo, Monaco, Consolas, monospace";

export function formatGbpMinor(valueMinor: number): string {
	const pounds = valueMinor / 100;
	return pounds.toLocaleString('en-GB', { style: 'currency', currency: 'GBP' });
}

export function createTreemapOption(data: TreemapDataItem[], colorScheme: ColorScheme = 'dark', compact = false): echarts.EChartsCoreOption {
	const colors = ECHARTS_COLORS[colorScheme];
	const fontSize = compact ? 9 : 11;

	return {
		tooltip: {
			trigger: 'item',
			backgroundColor: colors.tooltip.background,
			borderColor: colors.tooltip.border,
			textStyle: {
				color: colors.tooltip.text,
				fontFamily: DEFAULT_FONT_FAMILY,
				fontSize: 12,
			},
		},
		series: [
			{
				type: 'treemap',
				roam: false,
				nodeClick: false,
				breadcrumb: { show: false },
				label: {
					show: true,
					fontFamily: DEFAULT_FONT_FAMILY,
					fontSize,
					color: colorScheme === 'dark' ? '#ffffff' : '#374151',
					textShadowColor: colorScheme === 'dark' ? 'rgba(0,0,0,0.3)' : 'rgba(255,255,255,0.5)',
					textShadowBlur: 2,
					formatter: (params: unknown) => {
						const p = params as { name: string; value: number };
						return `${p.name}\n${formatGbpMinor(p.value)}`;
					},
				},
				upperLabel: {
					show: true,
					height: compact ? 20 : 28,
					fontFamily: DEFAULT_FONT_FAMILY,
					fontSize,
					color: colorScheme === 'dark' ? '#ffffff' : '#374151',
					textShadowColor: colorScheme === 'dark' ? 'rgba(0,0,0,0.3)' : 'rgba(255,255,255,0.5)',
					textShadowBlur: 2,
				},
				itemStyle: {
					borderColor: colorScheme === 'dark' ? '#1b1e22' : '#ffffff',
					borderWidth: compact ? 1 : 2,
					gapWidth: compact ? 1 : 2,
				},
				levels: [
					{
						itemStyle: {
							borderWidth: compact ? 2 : 3,
							gapWidth: compact ? 2 : 3,
						},
						upperLabel: {
							show: false,
						},
					},
					{
						itemStyle: {
							borderColorSaturation: colorScheme === 'dark' ? 0.2 : 0.9,
							gapWidth: 1,
						},
					},
					{
						itemStyle: {
							borderColorSaturation: colorScheme === 'dark' ? 0.2 : 0.9,
							gapWidth: 1,
						},
					},
				],
				data,
			},
		],
		color: TREEMAP_PALETTE[colorScheme],
	};
}

export function createSankeyOption(nodes: SankeyNode[], links: SankeyLink[], colorScheme: ColorScheme = 'dark', compact = false): echarts.EChartsCoreOption {
	const colors = ECHARTS_COLORS[colorScheme];
	const fontSize = compact ? 9 : 11;

	return {
		tooltip: {
			trigger: 'item',
			triggerOn: 'mousemove',
			backgroundColor: colors.tooltip.background,
			borderColor: colors.tooltip.border,
			textStyle: {
				color: colors.tooltip.text,
				fontFamily: DEFAULT_FONT_FAMILY,
				fontSize: 12,
			},
			formatter: (params: unknown) => {
				const p = params as { dataType: string; name?: string; data?: { source: string; target: string; value: number } };
				if (p.dataType === 'edge' && p.data) {
					return `${p.data.source} → ${p.data.target}<br/><strong>${formatGbpMinor(p.data.value)}</strong>`;
				}
				return p.name ?? '';
			},
		},
		animation: false,
		series: [
			{
				type: 'sankey',
				layout: 'none',
				emphasis: {
					focus: 'adjacency',
				},
				nodeAlign: 'left',
				orient: 'horizontal',
				left: compact ? 60 : 100,
				right: compact ? 80 : 150,
				top: compact ? 5 : 10,
				bottom: compact ? 5 : 10,
				nodeGap: compact ? 8 : 12,
				nodeWidth: compact ? 14 : 20,
				draggable: false,
				label: {
					fontFamily: DEFAULT_FONT_FAMILY,
					fontSize,
					color: colors.text,
					position: 'right',
				},
				lineStyle: {
					color: 'source',
					curveness: 0.5,
					opacity: 0.4,
				},
				itemStyle: {
					borderWidth: 0,
				},
				data: nodes,
				links,
			},
		],
	};
}

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

// ============================================================================
// Line Chart Types and Options
// ============================================================================

export type LineChartDataPoint = [string | number, number]; // [time, value]

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

// Semantic colors for income/expense visualizations (color-blind safe: teal/orange)
export const LINE_SEMANTIC_COLORS = {
	light: {
		income: '#0d9488', // teal-600
		incomeMuted: 'rgba(13, 148, 136, 0.6)',
		incomeFill: 'rgba(13, 148, 136, 0.28)',
		incomeFillFaint: 'rgba(13, 148, 136, 0.04)',
		expense: '#ea580c', // orange-600
		expenseMuted: 'rgba(234, 88, 12, 0.6)',
		expenseFill: 'rgba(234, 88, 12, 0.28)',
		expenseFillFaint: 'rgba(234, 88, 12, 0.04)',
		warning: 'rgba(217, 119, 6, 0.8)', // amber-700
		neutral: '#e6e6e8',
		neutralMuted: 'rgba(230, 230, 232, 0.16)',
	},
	dark: {
		income: '#2dd4bf', // teal-400
		incomeMuted: 'rgba(45, 212, 191, 0.6)',
		incomeFill: 'rgba(45, 212, 191, 0.28)',
		incomeFillFaint: 'rgba(45, 212, 191, 0.04)',
		expense: '#fb923c', // orange-400
		expenseMuted: 'rgba(251, 146, 60, 0.6)',
		expenseFill: 'rgba(251, 146, 60, 0.28)',
		expenseFillFaint: 'rgba(251, 146, 60, 0.04)',
		warning: 'rgba(240, 180, 41, 0.8)', // amber-400
		neutral: '#e6e6e8',
		neutralMuted: 'rgba(230, 230, 232, 0.16)',
	},
} as const;

function mapLineStyle(style?: 'solid' | 'dashed' | 'dotted'): 'solid' | 'dashed' | 'dotted' {
	return style ?? 'solid';
}

export function createLineChartOption(series: LineSeriesDefinition[], options: LineChartOptions = {}): echarts.EChartsCoreOption {
	const { colorScheme = 'dark', compact = false, showTooltip = true, formatTooltip, formatYAxis, markLines = [], xAxisType = 'time' } = options;

	const colors = ECHARTS_COLORS[colorScheme];

	return {
		animation: false,
		grid: {
			left: compact ? 8 : 60,
			right: compact ? 8 : 20,
			top: compact ? 8 : 20,
			bottom: compact ? 8 : 30,
			containLabel: !compact,
		},
		tooltip: showTooltip
			? {
					trigger: 'axis',
					backgroundColor: colors.tooltip.background,
					borderColor: colors.tooltip.border,
					textStyle: {
						color: colors.tooltip.text,
						fontFamily: DEFAULT_FONT_FAMILY,
						fontSize: 12,
					},
					formatter: formatTooltip,
					axisPointer: {
						type: 'cross',
						lineStyle: {
							color: colors.border,
							type: 'dotted',
						},
						crossStyle: {
							color: colors.border,
						},
						label: {
							backgroundColor: colorScheme === 'dark' ? '#1b1e22' : '#f3f4f6',
							color: colors.text,
							fontFamily: DEFAULT_FONT_FAMILY,
							fontSize: 11,
						},
					},
				}
			: undefined,
		xAxis: {
			type: xAxisType,
			show: !compact,
			axisLine: { show: false },
			axisTick: { show: false },
			axisLabel: {
				color: colors.textMuted,
				fontFamily: DEFAULT_FONT_FAMILY,
				fontSize: 11,
			},
			splitLine: {
				show: !compact,
				lineStyle: {
					color: colors.border,
					type: 'dotted',
				},
			},
		},
		yAxis: {
			type: 'value',
			show: !compact,
			axisLine: { show: false },
			axisTick: { show: false },
			axisLabel: {
				color: colors.textMuted,
				fontFamily: DEFAULT_FONT_FAMILY,
				fontSize: 11,
				formatter: formatYAxis,
			},
			splitLine: {
				show: true,
				lineStyle: {
					color: colors.border,
					type: 'dotted',
				},
			},
		},
		series: series.map((s, index) => ({
			type: 'line',
			name: s.key,
			data: s.data,
			smooth: s.smooth ?? true,
			symbol: s.showSymbol === false ? 'none' : 'circle',
			symbolSize: compact ? 0 : 4,
			lineStyle: {
				color: s.color,
				width: s.lineWidth ?? (compact ? 1 : 2),
				type: mapLineStyle(s.lineStyle),
			},
			itemStyle: {
				color: s.color,
			},
			areaStyle: s.areaStyle,
			// Add markLines only to the first series
			markLine:
				index === 0 && markLines.length > 0
					? {
							silent: true,
							symbol: 'none',
							data: markLines.map((ml) => ({
								yAxis: ml.yAxis,
								name: ml.name,
								label: {
									formatter: ml.label?.formatter ?? ml.name ?? '',
									position: ml.label?.position ?? 'end',
									color: colors.textMuted,
									fontFamily: DEFAULT_FONT_FAMILY,
									fontSize: 11,
								},
								lineStyle: {
									color: ml.lineStyle?.color ?? colors.textMuted,
									type: ml.lineStyle?.type ?? 'dashed',
									width: ml.lineStyle?.width ?? 1,
								},
							})),
						}
					: undefined,
		})),
	};
}

// Helper to convert data from {time, value} format to eCharts [time, value] format
export function toEchartsLineData<T>(data: T[], getTime: (item: T) => string, getValue: (item: T) => number): LineChartDataPoint[] {
	return data.map((item) => [getTime(item), getValue(item)]);
}
