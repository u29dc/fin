<script lang="ts">
	import { onMount } from 'svelte';
	import { loadEchartsRuntime, type EChartsType, type EchartsRuntime } from './runtime';
	import type { ColorScheme, SankeyLink, SankeyNode } from './types';

	type Props = {
		nodes: SankeyNode[];
		links: SankeyLink[];
		colorScheme?: ColorScheme;
		compact?: boolean;
	};

	const { nodes, links, colorScheme = 'dark', compact = false }: Props = $props();

	let container: HTMLDivElement;
	let chart: EChartsType | null = null;
	let runtime: EchartsRuntime | null = $state(null);

	function initChart() {
		let resizeObserver: ResizeObserver | null = null;
		let disposed = false;

		void (async () => {
			if (!container) return;

			runtime = await loadEchartsRuntime();
			if (disposed) {
				return;
			}

			chart = runtime.echarts.init(container, undefined, { renderer: 'canvas' });
			updateChart();

			resizeObserver = new ResizeObserver(() => {
				chart?.resize();
			});
			resizeObserver.observe(container);
		})();

		return () => {
			disposed = true;
			resizeObserver?.disconnect();
			chart?.dispose();
			chart = null;
			runtime = null;
		};
	}

	function updateChart() {
		if (!chart || !runtime) return;

		const option = runtime.createSankeyOption(nodes, links, colorScheme, compact);
		chart.setOption(option, true);
	}

	onMount(() => {
		return initChart();
	});

	$effect(() => {
		// Re-render when data, colorScheme, or compact changes
		nodes;
		links;
		colorScheme;
		compact;
		updateChart();
	});
</script>

<div bind:this={container} class="w-full h-full"></div>
