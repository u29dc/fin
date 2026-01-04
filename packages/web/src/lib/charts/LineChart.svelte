<script lang="ts" generics="T">
	import { onMount } from 'svelte';

	import { theme } from '$lib/theme.svelte';
	import {
		echarts,
		createLineChartOption,
		toEchartsLineData,
		ECHARTS_COLORS,
		DEFAULT_FONT_FAMILY,
		type LineSeriesDefinition,
	} from './echarts';

	type Props = {
		data: T[];
		getValue: (point: T) => number;
		getDate: (point: T) => string;
		formatValue: (value: number) => string;
		formatLocalization?: (value: number) => string;
		height?: number;
		compact?: boolean;
		lineColor?: string;
		downsample?: boolean;
		curve?: boolean;
		showRawOverlay?: boolean;
	};

	let {
		data,
		getValue,
		getDate,
		formatValue,
		formatLocalization,
		height = 220,
		compact = false,
		lineColor = '#e6e6e8',
		downsample = true,
		curve = true,
		showRawOverlay = true,
	}: Props = $props();

	let container: HTMLDivElement | null = $state(null);
	let chart: ReturnType<typeof echarts.init> | null = null;
	let hoverValueLabel: string | null = $state(null);

	const colorScheme = $derived(theme.resolved);

	function buildChartOption() {
		const colors = ECHARTS_COLORS[colorScheme];
		const chartData = toEchartsLineData(data, getDate, getValue);

		const series: LineSeriesDefinition[] = [];

		// Raw overlay series (semi-transparent) - same data, different styling
		if (!compact && showRawOverlay) {
			series.push({
				key: 'raw',
				data: chartData,
				color: 'rgba(230, 230, 232, 0.16)',
				lineWidth: 1,
				smooth: false,
				showSymbol: false,
			});
		}

		// Main series
		series.push({
			key: 'main',
			data: chartData,
			color: lineColor,
			lineWidth: compact ? 1 : 2,
			smooth: curve,
			showSymbol: false,
		});

		const option = createLineChartOption(series, {
			colorScheme,
			compact,
			showTooltip: !compact,
			formatYAxis: formatLocalization ?? ((value: number) => formatValue(value)),
			formatTooltip: (params: unknown) => {
				const p = params as { data?: [string, number] }[];
				// Get the main series data (last item if we have raw overlay)
				const mainData = p[p.length - 1]?.data;
				if (mainData) {
					const [time, value] = mainData;
					const dateStr = new Date(time).toLocaleDateString('en-GB', {
						day: 'numeric',
						month: 'short',
						year: 'numeric',
					});
					hoverValueLabel = formatValue(value);
					return `<span style="font-family: ${DEFAULT_FONT_FAMILY}; font-size: 11px;">${dateStr}<br/><strong>${formatValue(value)}</strong></span>`;
				}
				return '';
			},
		});

		return option;
	}

	function render() {
		if (!chart || data.length === 0) {
			return;
		}

		chart.setOption(buildChartOption(), true);
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

		// Handle mouse leave to clear hover label
		container.addEventListener('mouseleave', () => {
			hoverValueLabel = null;
		});

		return () => {
			resizeObserver.disconnect();
			chart?.dispose();
			chart = null;
		};
	});

	// Re-render when data or options change
	$effect(() => {
		if (data && lineColor && colorScheme) {
			render();
		}
	});
</script>

<div class="chart-wrap">
	<div class="hover-label" class:visible={hoverValueLabel !== null}>{hoverValueLabel ?? ''}</div>
	<div bind:this={container} class="chart-inner" style:height={height + 'px'}></div>
</div>

<style>
	.chart-wrap {
		position: relative;
		width: 100%;
	}

	.chart-inner {
		width: 100%;
	}

	.hover-label {
		position: absolute;
		top: 8px;
		right: 8px;
		font-family: 'JetBrains Mono', monospace;
		font-size: 12px;
		color: var(--text-secondary);
		opacity: 0;
		transition: opacity 0.15s ease;
		pointer-events: none;
		z-index: 10;
	}

	.hover-label.visible {
		opacity: 1;
	}
</style>
