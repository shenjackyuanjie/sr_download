'use strict';

(() => {
    const storageKey = 'theme-preference';
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

    function storedTheme() {
        try {
            const value = localStorage.getItem(storageKey);
            return value === 'light' || value === 'dark' ? value : null;
        } catch {
            return null;
        }
    }

    function storeTheme(theme) {
        try {
            localStorage.setItem(storageKey, theme);
        } catch {
            // The selected theme still applies for this page when storage is unavailable.
        }
    }

    function resolvedTheme() {
        return storedTheme() || (mediaQuery.matches ? 'dark' : 'light');
    }

    function applyTheme(theme) {
        document.documentElement.dataset.theme = theme;
        document.documentElement.style.colorScheme = theme;
        const button = document.getElementById('theme-toggle');
        if (!button) return;
        const nextThemeLabel = theme === 'dark' ? '浅色' : '深色';
        button.textContent = nextThemeLabel + '模式';
        button.setAttribute('aria-label', '切换到' + nextThemeLabel + '模式');
        button.title = '切换到' + nextThemeLabel + '模式';
    }

    applyTheme(resolvedTheme());

    document.addEventListener('DOMContentLoaded', () => {
        applyTheme(resolvedTheme());
        document.getElementById('theme-toggle')?.addEventListener('click', () => {
            const nextTheme = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
            storeTheme(nextTheme);
            applyTheme(nextTheme);
        });
    });

    mediaQuery.addEventListener('change', () => {
        if (!storedTheme()) applyTheme(resolvedTheme());
    });
})();
