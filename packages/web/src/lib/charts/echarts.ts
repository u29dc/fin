import { LineChart, SankeyChart, TreemapChart } from 'echarts/charts';
import { DataZoomComponent, GridComponent, LegendComponent, MarkLineComponent, TitleComponent, TooltipComponent, VisualMapComponent } from 'echarts/components';
import * as echarts from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';

import { DEFAULT_FONT_FAMILY, ECHARTS_COLORS, TREEMAP_PALETTE } from './palette';
import type { ColorScheme, LineChartDataPoint, LineChartOptions, LineSeriesDefinition, SankeyLink, SankeyNode, TreemapDataItem } from './types';

// Register required components
echarts.use([LineChart, TreemapChart, SankeyChart, CanvasRenderer, GridComponent, TooltipComponent, TitleComponent, LegendComponent, DataZoomComponent, MarkLineComponent, VisualMapComponent]);

export { echarts };
export type { ColorScheme, LineChartDataPoint, LineChartOptions, LineSeriesDefinition, MarkLineItem, SankeyLink, SankeyNode, TreemapDataItem } from './types';
export { DEFAULT_FONT_FAMILY, ECHARTS_COLORS, LINE_SEMANTIC_COLORS, SANKEY_PALETTE, TREEMAP_PALETTE } from './palette';

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
		color: [...TREEMAP_PALETTE[colorScheme]],
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
