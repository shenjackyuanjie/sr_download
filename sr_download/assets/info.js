'use strict';

class DataQueryManager {
    constructor() {
        this.isLoading = false;
        this.init();
    }

    init() {
        this.bindEvents();
        this.setupEnterKeyHandler();
    }

    bindEvents() {
        const queryButton = document.querySelector('.query-button');
        const queryInput = document.getElementById('dataId');

        if (queryButton) {
            queryButton.addEventListener('click', () => this.fetchData());
        }

        if (queryInput) {
            queryInput.addEventListener('input', (e) => this.validateInput(e.target));
        }
    }

    setupEnterKeyHandler() {
        const queryInput = document.getElementById('dataId');
        if (queryInput) {
            queryInput.addEventListener('keypress', (e) => {
                if (e.key === 'Enter' && !this.isLoading) {
                    this.fetchData();
                }
            });
        }
    }

    validateInput(input) {
        const value = parseInt(input.value);
        const minId = 76858;

        // 清除之前的错误样式
        input.classList.remove('error');

        if (input.value && (isNaN(value) || value < minId)) {
            input.classList.add('error');
            this.showInputError(`ID 必须是不小于 ${minId} 的数字`);
        } else {
            this.hideInputError();
        }
    }

    showInputError(message) {
        let errorDiv = document.querySelector('.input-error');
        if (!errorDiv) {
            errorDiv = document.createElement('div');
            errorDiv.className = 'input-error';
            errorDiv.style.color = 'var(--color-danger)';
            errorDiv.style.fontSize = '0.875rem';
            errorDiv.style.marginTop = 'var(--spacing-xs)';

            const inputGroup = document.querySelector('.input-group');
            if (inputGroup) {
                inputGroup.parentNode.insertBefore(errorDiv, inputGroup.nextSibling);
            }
        }
        errorDiv.textContent = message;
        errorDiv.style.display = 'block';
    }

    hideInputError() {
        const errorDiv = document.querySelector('.input-error');
        if (errorDiv) {
            errorDiv.style.display = 'none';
        }
    }

    async fetchData() {
        if (this.isLoading) return;

        const dataId = document.getElementById('dataId').value;
        const minId = 76858;

        // 输入验证
        if (!dataId) {
            this.showNotification('请输入 ID', 'error');
            return;
        }

        const numId = parseInt(dataId);
        if (isNaN(numId) || numId < minId) {
            this.showNotification(`ID 必须是不小于 ${minId} 的数字`, 'error');
            return;
        }

        this.setLoadingState(true);
        this.hideInputError();

        try {
            const response = await fetch(`/info/${dataId}`);
            const data = await response.json();

            if (data.code === 200) {
                this.displayResult(data.data, 'success');
                this.showNotification('数据获取成功', 'success');
            } else {
                this.displayResult(data.msg, 'error');
                this.showNotification(data.msg || '获取数据失败', 'error');
            }
        } catch (error) {
            console.error('请求失败:', error);
            this.displayResult('网络请求失败，请稍后重试', 'error');
            this.showNotification('网络请求失败，请稍后重试', 'error');
        } finally {
            this.setLoadingState(false);
        }
    }

    setLoadingState(loading) {
        this.isLoading = loading;
        const button = document.querySelector('.query-button');
        const input = document.getElementById('dataId');

        if (button) {
            button.disabled = loading;
            button.innerHTML = loading ?
                '<span class="loading-spinner"></span> 查询中...' :
                '查询数据';
        }

        if (input) {
            input.disabled = loading;
        }

        if (loading) {
            this.showLoadingResult();
        }
    }

    showLoadingResult() {
        const resultDisplay = document.getElementById('resultDisplay');
        if (!resultDisplay) return;

        resultDisplay.innerHTML = `
            <div class="result-loading">
                <div class="loading-spinner"></div>
                <span>正在获取数据...</span>
            </div>
        `;
    }

    displayResult(data, type) {
        const resultDisplay = document.getElementById('resultDisplay');
        if (!resultDisplay) return;

        resultDisplay.innerHTML = '';

        if (type === 'success') {
            const resultContent = this.createSuccessResult(data);
            resultDisplay.appendChild(resultContent);
        } else if (type === 'error') {
            const errorContent = this.createErrorResult(data);
            resultDisplay.appendChild(errorContent);
        }
    }

    createSuccessResult(data) {
        const container = document.createElement('div');
        container.className = 'result-success';

        const title = document.createElement('h3');
        title.textContent = '查询成功';
        title.style.marginBottom = 'var(--spacing-md)';
        title.style.color = 'var(--color-success)';
        container.appendChild(title);

        const dataContainer = document.createElement('ul'); // Changed from div to ul
        dataContainer.className = 'result-content';

        const dataItems = [
            { label: 'ID', value: data.save_id },
            { label: '类型', value: data.save_type },
            { label: '长度', value: data.len },
            { label: 'XML校验', value: data.xml_tested, isStatus: true },
            { label: 'Blake Hash', value: data.blake_hash, isHash: true }
        ];

        dataItems.forEach(item => {
            const dataItem = document.createElement('li'); // Changed from div to li
            dataItem.className = 'data-item';

            const label = document.createElement('span');
            label.className = 'data-label';
            label.textContent = item.label;

            // hash显示/隐藏逻辑
            if (item.isHash) {
                const value = document.createElement('span');
                value.className = 'data-value hash-value';
                // 默认截断显示
                const fullHash = item.value || '';
                const shortHash = fullHash.length > 16 ? (fullHash.substring(0, 8) + '...' + fullHash.substring(fullHash.length - 8)) : fullHash;
                value.textContent = shortHash;
                value.title = fullHash;

                // 按钮
                const btn = document.createElement('button');
                btn.type = 'button';
                btn.textContent = '显示全部';
                btn.style.marginLeft = '8px';
                btn.style.fontSize = '0.75rem';
                btn.style.padding = '2px 8px';
                btn.style.borderRadius = '6px';
                btn.style.border = '1px solid var(--color-border)';
                btn.style.background = 'var(--color-button-bg)';
                btn.style.color = 'var(--color-button-text)';
                btn.style.cursor = 'pointer';
                btn.style.transition = 'background 0.2s';
                btn.onmouseenter = () => btn.style.background = 'var(--color-button-hover)';
                btn.onmouseleave = () => btn.style.background = 'var(--color-button-bg)';

                let expanded = false;
                btn.onclick = () => {
                    expanded = !expanded;
                    if (expanded) {
                        value.textContent = fullHash;
                        value.classList.add('expanded');
                        btn.textContent = '隐藏';
                    } else {
                        value.textContent = shortHash;
                        value.classList.remove('expanded');
                        btn.textContent = '显示全部';
                    }
                };

                const valueWrap = document.createElement('span');
                valueWrap.style.display = 'inline-flex';
                valueWrap.style.alignItems = 'center';
                valueWrap.appendChild(value);
                valueWrap.appendChild(btn);

                dataItem.appendChild(label);
                dataItem.appendChild(valueWrap);
            } else {
                const value = document.createElement('span');
                value.className = 'data-value';
                if (item.isStatus) {
                    value.className += ' status-badge';
                    const status = item.value ? 'ok' : 'failed';
                    value.setAttribute('data-status', status);
                    const displayText = item.value ? 'PASSED' : 'FAILED';
                    value.textContent = displayText;
                } else {
                    value.textContent = item.value;
                }
                dataItem.appendChild(label);
                dataItem.appendChild(value);
            }
            dataContainer.appendChild(dataItem);
        });

        container.appendChild(dataContainer);
        return container;
    }

    createErrorResult(message) {
        const container = document.createElement('div');
        container.className = 'result-error';

        const title = document.createElement('h3');
        title.textContent = '查询失败';
        title.style.marginBottom = 'var(--spacing-md)';
        container.appendChild(title);

        const errorMessage = document.createElement('p');
        errorMessage.textContent = message;
        errorMessage.style.margin = '0';
        container.appendChild(errorMessage);

        return container;
    }

    showNotification(message, type = 'info') {
        // 移除现有通知
        const existingNotification = document.querySelector('.notification');
        if (existingNotification) {
            existingNotification.remove();
        }

        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.textContent = message;

        // 样式
        Object.assign(notification.style, {
            position: 'fixed',
            top: '20px',
            right: '20px',
            padding: 'var(--spacing-md) var(--spacing-lg)',
            borderRadius: 'var(--radius-md)',
            color: 'white',
            fontWeight: '500',
            fontSize: '0.875rem',
            zIndex: '9999',
            boxShadow: 'var(--shadow-lg)',
            transform: 'translateX(100%)',
            transition: 'transform 0.3s ease',
            maxWidth: '300px',
            wordWrap: 'break-word'
        });

        // 根据类型设置背景色
        switch (type) {
            case 'success':
                notification.style.backgroundColor = 'var(--color-success)';
                break;
            case 'error':
                notification.style.backgroundColor = 'var(--color-danger)';
                break;
            case 'warning':
                notification.style.backgroundColor = 'var(--color-warning)';
                break;
            default:
                notification.style.backgroundColor = 'var(--color-info)';
        }

        document.body.appendChild(notification);

        // 动画显示
        setTimeout(() => {
            notification.style.transform = 'translateX(0)';
        }, 100);

        // 自动隐藏
        setTimeout(() => {
            notification.style.transform = 'translateX(100%)';
            setTimeout(() => {
                if (notification.parentNode) {
                    notification.parentNode.removeChild(notification);
                }
            }, 300);
        }, 3000);
    }
}

// 数据格式化工具
class DataFormatter {
    static formatFileSize(bytes) {
        if (bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    }

    static formatNumber(num) {
        return num.toLocaleString();
    }

    static formatTime(timestamp) {
        const date = new Date(timestamp);
        return date.toLocaleString('zh-CN', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });
    }

    static truncateHash(hash, length = 8) {
        if (!hash || hash.length <= length * 2) return hash;
        return hash.substring(0, length) + '...' + hash.substring(hash.length - length);
    }
}

// 主题切换增强
class ThemeManager {
    constructor() {
        this.init();
    }

    init() {
        this.updateThemeBasedContent();
        this.observeThemeChanges();
    }

    updateThemeBasedContent() {
        const isDark = document.documentElement.getAttribute('data-theme') === 'dark';

        // 更新状态徽章样式
        const statusBadges = document.querySelectorAll('.status-badge');
        statusBadges.forEach(badge => {
            const isTrue = badge.getAttribute('data-status') === 'true';
            if (isTrue) {
                badge.style.backgroundColor = 'var(--color-success)';
                badge.style.color = 'white';
            } else {
                badge.style.backgroundColor = 'var(--color-danger)';
                badge.style.color = 'white';
            }
        });
    }

    observeThemeChanges() {
        const observer = new MutationObserver((mutations) => {
            mutations.forEach((mutation) => {
                if (mutation.type === 'attributes' && mutation.attributeName === 'data-theme') {
                    this.updateThemeBasedContent();
                }
            });
        });

        observer.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ['data-theme']
        });
    }
}

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', () => {
    new DataQueryManager();
    new ThemeManager();
    enhanceAllHashValues();
    // 添加页面加载完成提示
    console.log('sr-download 信息面板已加载完成 powered by AI');
});

/**
 * 让所有页面上的 .hash-value 都支持截断和显示完整hash的按钮
 */
function enhanceAllHashValues() {
    // 只处理未被增强过的hash-value
    document.querySelectorAll('.hash-value').forEach(function (el) {
        // 避免重复增强
        if (el.dataset.updated === '1') return;
        el.dataset.updated = '1';

        const fullHash = el.textContent.trim();
        if (!fullHash || fullHash.length <= 16) return; // 不需要截断

        const shortHash = fullHash.substring(0, 8) + '...' + fullHash.substring(fullHash.length - 8);

        // 创建包裹
        const valueWrap = document.createElement('span');
        valueWrap.style.display = 'inline-flex';
        valueWrap.style.alignItems = 'center';

        // 创建hash显示span
        const hashSpan = document.createElement('span');
        hashSpan.className = 'hash-value';
        hashSpan.textContent = shortHash;
        hashSpan.title = fullHash;

        // 创建按钮
        const btn = document.createElement('button');
        btn.type = 'button';
        btn.textContent = '显示全部';
        btn.style.marginLeft = '8px';
        btn.style.fontSize = '0.75rem';
        btn.style.padding = '2px 8px';
        btn.style.borderRadius = '6px';
        btn.style.border = '1px solid var(--color-border)';
        btn.style.background = 'var(--color-button-bg)';
        btn.style.color = 'var(--color-button-text)';
        btn.style.cursor = 'pointer';
        btn.style.transition = 'background 0.2s';
        btn.onmouseenter = () => btn.style.background = 'var(--color-button-hover)';
        btn.onmouseleave = () => btn.style.background = 'var(--color-button-bg)';

        let expanded = false;
        btn.onclick = () => {
            expanded = !expanded;
            if (expanded) {
                hashSpan.textContent = fullHash;
                hashSpan.classList.add('expanded');
                btn.textContent = '隐藏';
            } else {
                hashSpan.textContent = shortHash;
                hashSpan.classList.remove('expanded');
                btn.textContent = '显示全部';
            }
        };

        valueWrap.appendChild(hashSpan);
        valueWrap.appendChild(btn);

        // 替换原有el
        el.replaceWith(valueWrap);
    });
    // 处理一下 status-badge
    document.querySelectorAll('.status-badge').forEach(function (el) {
      // AI 这么干的, 我觉得有道理, 所以也这么干了
      if (el.dataset.updated === '1') return;
      el.dataset.updated = '1';

      const original_value = el.getAttribute('data-status');
      let display_value = "FAILED";
      let bg_color = 'var(--color-status-failed)';
      let text_color = 'var(--color-status-failed-text)';

      switch (original_value) {
        case 'ok':
          display_value = "PASSED";
          bg_color = 'var(--color-status-passed)';
          text_color = 'var(--color-status-passed-text)';
          break;
        case 'no data':
          display_value = "NO DATA";
          bg_color = 'var(--color-status-nodata)';
          text_color = 'var(--color-status-nodata-text)';
          break;
        default:
          display_value = original_value.toUpperCase();
          break;
      }

      // 创建样式化的状态标签
      const badge = document.createElement('span');
      badge.textContent = display_value;
      badge.style.display = 'inline-block';
      badge.style.padding = '2px 8px';
      badge.style.borderRadius = '12px';
      badge.style.fontSize = '0.75rem';
      badge.style.fontWeight = 'bold';
      badge.style.backgroundColor = bg_color;
      badge.style.color = text_color;
      badge.style.textTransform = 'uppercase';
      badge.style.letterSpacing = '0.5px';
      badge.title = `Status: ${original_value}`;

      // 替换原有元素
      el.replaceWith(badge);
    })
}

// 全局函数保持向后兼容
window.fetchData = function() {
    const manager = new DataQueryManager();
    manager.fetchData();
};
