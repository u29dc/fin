import type { ColorScheme } from "./types";

export const ECHARTS_COLORS = {
	light: {
		background: "transparent",
		text: "#374151",
		textMuted: "#6b7280",
		border: "rgba(0, 0, 0, 0.08)",
		tooltip: {
			background: "#ffffff",
			border: "#e5e7eb",
			text: "#374151",
		},
	},
	dark: {
		background: "transparent",
		text: "#e6e6e8",
		textMuted: "#9aa0a6",
		border: "rgba(230, 230, 232, 0.08)",
		tooltip: {
			background: "#1b1e22",
			border: "#2d3139",
			text: "#e6e6e8",
		},
	},
} as const satisfies Record<
	ColorScheme,
	{
		background: string;
		text: string;
		textMuted: string;
		border: string;
		tooltip: {
			background: string;
			border: string;
			text: string;
		};
	}
>;

export const TREEMAP_PALETTE = {
	light: ["#e2e8f0", "#d8dfe8", "#ced6e0", "#c4ccd6", "#bac3cd"],
	dark: ["#2d3139", "#282d34", "#23282f", "#1e2329", "#1a1e24"],
} as const satisfies Record<ColorScheme, readonly string[]>;

export const SANKEY_PALETTE = {
	light: {
		income: "#0d9488",
		asset: "#94a3b8",
		expense: "#94a3b8",
	},
	dark: {
		income: "#2dd4bf",
		asset: "#64748b",
		expense: "#64748b",
	},
} as const satisfies Record<
	ColorScheme,
	{
		income: string;
		asset: string;
		expense: string;
	}
>;

export const DEFAULT_FONT_FAMILY =
	"'JetBrains Mono', 'SFMono-Regular', Menlo, Monaco, Consolas, monospace";

export const LINE_SEMANTIC_COLORS = {
	light: {
		income: "#0d9488",
		incomeMuted: "rgba(13, 148, 136, 0.6)",
		incomeFill: "rgba(13, 148, 136, 0.28)",
		incomeFillFaint: "rgba(13, 148, 136, 0.04)",
		expense: "#ea580c",
		expenseMuted: "rgba(234, 88, 12, 0.6)",
		expenseFill: "rgba(234, 88, 12, 0.28)",
		expenseFillFaint: "rgba(234, 88, 12, 0.04)",
		warning: "rgba(217, 119, 6, 0.8)",
		neutral: "#e6e6e8",
		neutralMuted: "rgba(230, 230, 232, 0.16)",
	},
	dark: {
		income: "#2dd4bf",
		incomeMuted: "rgba(45, 212, 191, 0.6)",
		incomeFill: "rgba(45, 212, 191, 0.28)",
		incomeFillFaint: "rgba(45, 212, 191, 0.04)",
		expense: "#fb923c",
		expenseMuted: "rgba(251, 146, 60, 0.6)",
		expenseFill: "rgba(251, 146, 60, 0.28)",
		expenseFillFaint: "rgba(251, 146, 60, 0.04)",
		warning: "rgba(240, 180, 41, 0.8)",
		neutral: "#e6e6e8",
		neutralMuted: "rgba(230, 230, 232, 0.16)",
	},
} as const;
