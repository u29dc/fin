<script lang="ts" generics="T">
	import { onMount } from 'svelte';

	import { theme } from '$lib/theme.svelte';
	import {
		echarts,
		createLineChartOption,
		ECHARTS_COLORS,
		DEFAULT_FONT_FAMILY,
		type LineSeriesDefinition,
		type MarkLineItem,
	} from './echarts';
	import { asRgba } from './utils';

	type SeriesDefinition = {
		key: string;
		color: string;
		lineStyle?: 'solid' | 'dashed' | 'dotted';
		lineWidth?: number;
		getValue: (point: T) => number;
		visible?: boolean;
		showInHover?: boolean;
		lastValueVisible?: boolean;
		priceLineVisible?: boolean;
		crosshairMarkerVisible?: boolean;
	};

	type ThresholdLine = {
		value: number;
		color: string;
		label?: string;
	};

	type Props = {
		data: T[];
		getDate: (point: T) => string;
		series: SeriesDefinition[];
		formatHover: (point: T) => string;
		height?: number;
		compact?: boolean;
		curve?: boolean;
		showRawOverlay?: boolean;
		timeUnit?: 'day' | 'month';
		thresholds?: ThresholdLine[];
	};

	let {
		data,
		getDate,
		series,
		formatHover,
		height = 180,
		compact = false,
		curve = true,
		showRawOverlay = true,
		timeUnit = 'day',
		thresholds = [],
	}: Props = $props();

	let container: HTMLDivElement | null = $state(null);
	let chart: ReturnType<typeof echarts.init> | null = null;

	const colorScheme = $derived(theme.resolved);

	// Memoize chart option - only recomputes when dependencies change
	const chartOption = $derived.by(() => {
		if (data.length === 0) {
			return null;
		}

		const echartsSeriesList: LineSeriesDefinition[] = [];

		// Build series in definition order
		for (const def of series) {
			if (def.visible === false) {
				continue;
			}

			const seriesData = data.map((p) => [getDate(p), def.getValue(p)] as [string, number]);

			// Add raw overlay series if enabled
			if (!compact && showRawOverlay) {
				echartsSeriesList.push({
					key: `${def.key}_raw`,
					data: seriesData,
					color: asRgba(def.color, 0.16),
					lineWidth: 1,
					smooth: false,
					lineStyle: def.lineStyle,
					showSymbol: false,
				});
			}

			// Add main series
			echartsSeriesList.push({
				key: def.key,
				data: seriesData,
				color: def.color,
				lineWidth: def.lineWidth ?? (compact ? 1 : 2),
				smooth: curve,
				lineStyle: def.lineStyle,
				showSymbol: false,
			});
		}

		// Convert thresholds to markLines
		const markLines: MarkLineItem[] = thresholds.map((t) => ({
			yAxis: t.value,
			name: t.label,
			label: {
				formatter: t.label ?? '',
				position: 'end' as const,
			},
			lineStyle: {
				color: t.color,
				type: 'dashed' as const,
				width: 1,
			},
		}));

		return createLineChartOption(echartsSeriesList, {
			colorScheme,
			compact,
			showTooltip: !compact,
			markLines,
			xAxisType: timeUnit === 'month' ? 'category' : 'time',
			formatTooltip: (params: unknown) => {
				const p = params as { dataIndex?: number }[];
				const dataIndex = p[0]?.dataIndex;
				if (dataIndex !== undefined && data[dataIndex]) {
					const point = data[dataIndex];
					return `<span style="font-family: ${DEFAULT_FONT_FAMILY}; font-size: 11px;">${formatHover(point)}</span>`;
				}
				return '';
			},
		});
	});

	function render() {
		if (!chart || !chartOption) {
			return;
		}

		chart.setOption(chartOption, true);
	}

	onMount(() => {
		if (!container) {
			return;
		}

		chart = echarts.init(container, undefined, { renderer: 'canvas' });
		render();

		const resizeObserver = new ResizeObserver(() => {
			chart?.resize();
		});
		resizeObserver.observe(container);

		return () => {
			resizeObserver.disconnect();
			chart?.dispose();
			chart = null;
		};
	});

	// Re-render when chartOption changes (memoized)
	$effect(() => {
		if (chartOption) {
			render();
		}
	});
</script>

<div bind:this={container} class="w-full" style:height={height + 'px'}></div>
