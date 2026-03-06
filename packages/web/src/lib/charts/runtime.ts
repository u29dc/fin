import type { EChartsCoreOption, EChartsType } from "echarts/core";

import type { ColorScheme, LineChartDataPoint, LineChartOptions, LineSeriesDefinition, SankeyLink, SankeyNode, TreemapDataItem } from "./types";

type ChartColors = {
	background: string;
	text: string;
	textMuted: string;
	border: string;
	tooltip: {
		background: string;
		border: string;
		text: string;
	};
};

type SemanticColors = {
	income: string;
	incomeMuted: string;
	incomeFill: string;
	incomeFillFaint: string;
	expense: string;
	expenseMuted: string;
	expenseFill: string;
	expenseFillFaint: string;
	warning: string;
	neutral: string;
	neutralMuted: string;
};

export type EchartsRuntime = {
	echarts: typeof import("echarts/core");
	ECHARTS_COLORS: Record<ColorScheme, ChartColors>;
	LINE_SEMANTIC_COLORS: Record<ColorScheme, SemanticColors>;
	DEFAULT_FONT_FAMILY: string;
	formatGbpMinor(valueMinor: number): string;
	createTreemapOption(data: TreemapDataItem[], colorScheme?: ColorScheme, compact?: boolean): EChartsCoreOption;
	createSankeyOption(nodes: SankeyNode[], links: SankeyLink[], colorScheme?: ColorScheme, compact?: boolean): EChartsCoreOption;
	createLineChartOption(series: LineSeriesDefinition[], options?: LineChartOptions): EChartsCoreOption;
	toEchartsLineData<T>(data: T[], getTime: (item: T) => string, getValue: (item: T) => number): LineChartDataPoint[];
};

let runtimePromise: Promise<EchartsRuntime> | null = null;

export function loadEchartsRuntime(): Promise<EchartsRuntime> {
	runtimePromise ??= import("$lib/charts/echarts") as Promise<EchartsRuntime>;
	return runtimePromise;
}

export type { EChartsType };
