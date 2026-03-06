import { browser } from '$app/environment';

type ThemeSetting = 'light' | 'dark' | 'system';
type ResolvedTheme = 'light' | 'dark';

const STORAGE_KEY = 'theme';

function getSystemPreference(): ResolvedTheme {
	if (!browser) {
		return 'light';
	}
	return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function getInitialSetting(): ThemeSetting {
	if (!browser) {
		return 'system';
	}
	try {
		const stored = localStorage.getItem(STORAGE_KEY);
		if (stored === 'light' || stored === 'dark' || stored === 'system') {
			return stored;
		}
	} catch {
		return 'system';
	}
	return 'system';
}

let setting = $state<ThemeSetting>(getInitialSetting());
let systemPreference = $state<ResolvedTheme>(getSystemPreference());

export const theme = {
	get setting() {
		return setting;
	},
	get resolved(): ResolvedTheme {
		return setting === 'system' ? systemPreference : setting;
	},
	cycle() {
		// system -> dark -> light -> system
		if (setting === 'system') {
			setting = 'dark';
		} else if (setting === 'dark') {
			setting = 'light';
		} else {
			setting = 'system';
		}
	},
};

if (browser) {
	// Listen for OS preference changes
	const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
	mediaQuery.addEventListener('change', (e) => {
		systemPreference = e.matches ? 'dark' : 'light';
	});

	// Sync setting to localStorage and HTML class
	$effect.root(() => {
		$effect(() => {
			try {
				if (setting === 'system') {
					localStorage.removeItem(STORAGE_KEY);
				} else {
					localStorage.setItem(STORAGE_KEY, setting);
				}
			} catch {
				// Ignore storage failures (e.g., private browsing)
			}

			const resolved = setting === 'system' ? systemPreference : setting;
			document.documentElement.classList.toggle('dark', resolved === 'dark');

			// Sync theme-color meta tag for browser chrome
			const themeColor = resolved === 'dark' ? '#0d0f10' : '#f8f9fa';
			document.querySelector('meta[name="theme-color"]')?.setAttribute('content', themeColor);
		});
	});
}
