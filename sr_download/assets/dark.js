(function() {
    'use strict';

    const storageKey = 'theme-preference';
    const themeAttribute = 'data-theme';

    // 获取当前主题
    function getThemePreference() {
        const saved = localStorage.getItem(storageKey);
        const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        return saved || (systemDark ? 'dark' : 'light');
    }

    // 切换主题
    function toggleTheme() {
        const current = document.documentElement.getAttribute(themeAttribute);
        const newTheme = current === 'dark' ? 'light' : 'dark';
        document.documentElement.setAttribute(themeAttribute, newTheme);
        localStorage.setItem(storageKey, newTheme);
        updateToggleButton(newTheme);
    }

    // 更新切换按钮
    function updateToggleButton(theme) {
        toggleButton.textContent = theme === 'dark' ? '🌞' : '🌙';
        toggleButton.setAttribute('aria-label', theme === 'dark' ? '切换到浅色模式' : '切换到深色模式');
        toggleButton.setAttribute('title', theme === 'dark' ? '切换到浅色模式' : '切换到深色模式');
    }

    // 创建切换按钮
    const toggleButton = document.createElement('button');
    toggleButton.className = 'theme-toggle';
    toggleButton.type = 'button';

    // 初始化
    function init() {
        const theme = getThemePreference();
        document.documentElement.setAttribute(themeAttribute, theme);
        updateToggleButton(theme);
        document.body.appendChild(toggleButton);
        toggleButton.addEventListener('click', toggleTheme);
    }

    // 系统主题变化监听
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
        if (!localStorage.getItem(storageKey)) {
            const theme = e.matches ? 'dark' : 'light';
            document.documentElement.setAttribute(themeAttribute, theme);
            updateToggleButton(theme);
        }
    });

    document.addEventListener('DOMContentLoaded', init);
})();
