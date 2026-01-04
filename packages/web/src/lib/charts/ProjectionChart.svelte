<script lang="ts">
	import { onMount } from 'svelte';

	import { theme } from '$lib/theme.svelte';
	import { echarts, ECHARTS_COLORS, DEFAULT_FONT_FAMILY, LINE_SEMANTIC_COLORS, formatGbpMinor } from './echarts';
	import type { ProjectionPoint } from './utils';

	type Props = {
		currentBurn: ProjectionPoint[];
		minimumBurn: ProjectionPoint[];
		threshold?: number;
		warningLine?: number;
		height?: number;
		formatHover?: (current: ProjectionPoint, minimum: ProjectionPoint) => string;
	};

	let {
		currentBurn,
		minimumBurn,
		threshold = 40_000,
		warningLine = 50_000,
		height = 320,
		formatHover,
	}: Props = $props();

	let container: HTMLDivElement | null = $state(null);
	let chart: ReturnType<typeof echarts.init> | null = null;
	let hoverValueLabel: string | null = $state(null);

	const colorScheme = $derived(theme.resolved);
	const colors = $derived(ECHARTS_COLORS[colorScheme]);
	const semantic = $derived(LINE_SEMANTIC_COLORS[colorScheme]);

	function buildChartOption() {
		// Convert data to [date, value] format (value in pounds, not pence)
		const currentData = currentBurn.map((p) => [p.date, p.balanceMinor / 100] as [string, number]);
		const minimumData = minimumBurn.map((p) => [p.date, p.balanceMinor / 100] as [string, number]);

		// Threshold in pounds for y-axis comparison
		const thresholdPounds = threshold;

		return {
			animation: false,
			grid: {
				left: 60,
				right: 20,
				top: 20,
				bottom: 30,
				containLabel: true,
			},
			tooltip: {
				trigger: 'axis',
				backgroundColor: colors.tooltip.background,
				borderColor: colors.tooltip.border,
				textStyle: {
					color: colors.tooltip.text,
					fontFamily: DEFAULT_FONT_FAMILY,
					fontSize: 12,
				},
				formatter: (params: unknown) => {
					const p = params as { dataIndex?: number }[];
					const dataIndex = p[0]?.dataIndex;
					if (dataIndex !== undefined && currentBurn[dataIndex] && minimumBurn[dataIndex]) {
						const current = currentBurn[dataIndex];
						const minimum = minimumBurn[dataIndex];
						if (formatHover) {
							hoverValueLabel = formatHover(current, minimum);
						} else {
							const dateStr = new Date(current.date).toLocaleDateString('en-GB', {
								month: 'short',
								year: 'numeric',
							});
							hoverValueLabel = `${dateStr} (Month ${current.month})\nCurrent: ${formatGbpMinor(current.balanceMinor)}\nMinimum: ${formatGbpMinor(minimum.balanceMinor)}`;
						}
						return `<span style="font-family: ${DEFAULT_FONT_FAMILY}; font-size: 11px; white-space: pre-line;">${hoverValueLabel}</span>`;
					}
					return '';
				},
				axisPointer: {
					type: 'cross',
					lineStyle: {
						color: colors.border,
						type: 'dotted',
					},
					label: {
						backgroundColor: colorScheme === 'dark' ? '#1b1e22' : '#f3f4f6',
						color: colors.text,
						fontFamily: DEFAULT_FONT_FAMILY,
						fontSize: 11,
					},
				},
			},
			xAxis: {
				type: 'time',
				axisLine: { show: false },
				axisTick: { show: false },
				axisLabel: {
					color: colors.textMuted,
					fontFamily: DEFAULT_FONT_FAMILY,
					fontSize: 11,
				},
				splitLine: {
					show: true,
					lineStyle: {
						color: colors.border,
						type: 'dotted',
					},
				},
			},
			yAxis: {
				type: 'value',
				axisLine: { show: false },
				axisTick: { show: false },
				axisLabel: {
					color: colors.textMuted,
					fontFamily: DEFAULT_FONT_FAMILY,
					fontSize: 11,
					formatter: (value: number) => formatGbpMinor(value * 100),
				},
				splitLine: {
					show: true,
					lineStyle: {
						color: colors.border,
						type: 'dotted',
					},
				},
			},
			series: [
				// Current burn series (solid, primary)
				{
					type: 'line',
					name: 'Current',
					data: currentData,
					smooth: false,
					symbol: 'none',
					lineStyle: {
						width: 2,
						color: semantic.income,
					},
					itemStyle: {
						color: semantic.income,
					},
					areaStyle: {
						color: semantic.incomeFill,
					},
					markLine: {
						silent: true,
						symbol: 'none',
						data: [
							{
								yAxis: warningLine,
								name: `${warningLine / 1000}K`,
								label: {
									formatter: `${warningLine / 1000}K`,
									position: 'end',
									color: colors.textMuted,
									fontFamily: DEFAULT_FONT_FAMILY,
									fontSize: 11,
								},
								lineStyle: {
									color: semantic.warning,
									type: 'dashed',
									width: 1,
								},
							},
							{
								yAxis: thresholdPounds,
								name: `${thresholdPounds / 1000}K`,
								label: {
									formatter: `${thresholdPounds / 1000}K`,
									position: 'end',
									color: colors.textMuted,
									fontFamily: DEFAULT_FONT_FAMILY,
									fontSize: 11,
								},
								lineStyle: {
									color: semantic.expense,
									type: 'dashed',
									width: 1,
								},
							},
						],
					},
				},
				// Minimum burn series (dashed, muted)
				{
					type: 'line',
					name: 'Minimum',
					data: minimumData,
					smooth: false,
					symbol: 'none',
					lineStyle: {
						width: 2,
						type: 'dashed',
						color: semantic.incomeMuted,
					},
					itemStyle: {
						color: semantic.incomeMuted,
					},
					areaStyle: {
						color: semantic.incomeFillFaint,
					},
				},
			],
		};
	}

	function render() {
		if (!chart || currentBurn.length === 0) {
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

		container.addEventListener('mouseleave', () => {
			hoverValueLabel = null;
		});

		return () => {
			resizeObserver.disconnect();
			chart?.dispose();
			chart = null;
		};
	});

	// Re-render when data or theme changes
	$effect(() => {
		if (currentBurn && minimumBurn && colorScheme) {
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
		white-space: pre-line;
	}

	.hover-label.visible {
		opacity: 1;
	}
</style>
