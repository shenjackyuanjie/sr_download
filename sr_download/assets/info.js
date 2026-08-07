'use strict';

const API = Object.freeze({
    overview: '/api/overview',
    record: (id) => '/api/records/' + id,
    raw: (id) => '/api/records/' + id + '/raw',
});

const DEFAULT_MIN_ID = 76858;
const REFRESH_INTERVAL_MS = 30000;
const VALID_TABS = new Set(['overview', 'record']);

class DashboardApp {
    constructor() {
        this.activeTab = 'overview';
        this.minLookupId = DEFAULT_MIN_ID;
        this.lastOverviewAt = 0;
        this.overviewData = null;
        this.currentRecord = null;
        this.xmlAnalysis = null;
        this.rawDisplayMode = 'formatted';
        this.rawWrap = false;
        this.refreshTimer = null;
        this.overviewController = null;
        this.lookupController = null;
        this.toastTimer = null;

        this.elements = {
            tabs: Array.from(document.querySelectorAll('[data-tab-target]')),
            panels: Array.from(document.querySelectorAll('[data-tab-panel]')),
            refresh: document.getElementById('refresh-overview'),
            overviewError: document.getElementById('overview-error'),
            latestRecords: document.getElementById('latest-records'),
            lookupForm: document.getElementById('lookup-form'),
            lookupInput: document.getElementById('record-id'),
            lookupSubmit: document.getElementById('lookup-submit'),
            lookupFeedback: document.getElementById('lookup-feedback'),
            queryPlaceholder: document.getElementById('query-placeholder'),
            recordDetail: document.getElementById('record-detail'),
            rawView: document.getElementById('record-raw-view'),
            formatToggle: document.getElementById('toggle-xml-format'),
            wrapToggle: document.getElementById('toggle-xml-wrap'),
            toast: document.getElementById('toast'),
        };
    }

    init() {
        this.bindEvents();

        const locationState = this.readLocation();
        this.setTab(locationState.tab, false);
        if (locationState.invalid) {
            this.writeLocation('overview', null, true);
        }

        this.loadOverview();

        if (locationState.tab === 'record' && locationState.id !== null) {
            this.elements.lookupInput.value = String(locationState.id);
            this.lookupRecord(locationState.id, { updateUrl: false });
        }
    }

    bindEvents() {
        this.elements.tabs.forEach((tab, index) => {
            tab.addEventListener('click', () => {
                if (tab.dataset.tabTarget !== this.activeTab) {
                    this.setTab(tab.dataset.tabTarget, true);
                }
            });
            tab.addEventListener('keydown', (event) => {
                if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
                event.preventDefault();
                const direction = event.key === 'ArrowRight' ? 1 : -1;
                const nextIndex = (index + direction + this.elements.tabs.length) % this.elements.tabs.length;
                const nextTab = this.elements.tabs[nextIndex];
                this.setTab(nextTab.dataset.tabTarget, true);
                nextTab.focus();
            });
        });

        this.elements.refresh.addEventListener('click', () => this.loadOverview());
        this.elements.lookupForm.addEventListener('submit', (event) => {
            event.preventDefault();
            this.lookupRecord();
        });

        this.elements.latestRecords.addEventListener('click', (event) => {
            const recordButton = event.target.closest('[data-record-id]');
            if (recordButton) {
                this.openRecord(Number(recordButton.dataset.recordId));
                return;
            }

            const copyButton = event.target.closest('[data-copy-value]');
            if (copyButton) {
                this.copyText(copyButton.dataset.copyValue, '哈希已复制');
            }
        });

        document.getElementById('copy-record-id').addEventListener('click', () => {
            if (this.currentRecord) {
                this.copyText(String(this.currentRecord.info.save_id), '记录 ID 已复制');
            }
        });
        document.getElementById('copy-record-hash').addEventListener('click', () => {
            if (this.currentRecord) {
                this.copyText(this.currentRecord.info.blake_hash, 'Blake3 哈希已复制');
            }
        });
        document.getElementById('copy-record-raw').addEventListener('click', () => {
            if (this.currentRecord?.raw_data) {
                this.copyText(this.currentRecord.raw_data, 'XML 原文已复制');
            }
        });
        document.getElementById('download-record').addEventListener('click', () => this.downloadRecord());
        this.elements.formatToggle.addEventListener('click', () => {
            if (!this.xmlAnalysis?.valid) return;
            this.rawDisplayMode = this.rawDisplayMode === 'formatted' ? 'raw' : 'formatted';
            this.renderRawView();
        });
        this.elements.wrapToggle.addEventListener('click', () => {
            this.rawWrap = !this.rawWrap;
            this.renderRawView();
        });

        document.addEventListener('visibilitychange', () => {
            if (document.hidden) {
                this.clearRefreshTimer();
                return;
            }
            if (this.activeTab !== 'overview') return;
            if (Date.now() - this.lastOverviewAt >= REFRESH_INTERVAL_MS) {
                this.loadOverview();
            } else {
                this.scheduleRefresh();
            }
        });

        window.addEventListener('popstate', () => {
            const locationState = this.readLocation();
            this.setTab(locationState.tab, false);
            if (locationState.tab !== 'record') return;

            if (locationState.id === null) {
                this.resetQuery();
            } else {
                this.elements.lookupInput.value = String(locationState.id);
                if (this.currentRecord?.info.save_id !== locationState.id) {
                    this.lookupRecord(locationState.id, { updateUrl: false });
                }
            }
        });
    }

    readLocation() {
        const params = new URLSearchParams(window.location.search);
        const rawTab = params.get('tab');
        const tab = VALID_TABS.has(rawTab) ? rawTab : 'overview';
        const rawId = params.get('id');

        if (rawId === null) {
            return { tab, id: null, invalid: rawTab !== null && !VALID_TABS.has(rawTab) };
        }

        const id = Number(rawId);
        const validId = Number.isInteger(id) && id >= DEFAULT_MIN_ID;
        if (tab !== 'record' || !validId) {
            return { tab: 'overview', id: null, invalid: true };
        }
        return { tab, id, invalid: false };
    }

    writeLocation(tab, id = null, replace = false) {
        const url = new URL(window.location.href);
        url.searchParams.delete('tab');
        url.searchParams.delete('id');
        if (tab === 'record') {
            url.searchParams.set('tab', 'record');
            if (id !== null) url.searchParams.set('id', String(id));
        }
        const method = replace ? 'replaceState' : 'pushState';
        window.history[method]({}, '', url);
    }

    setTab(tab, updateUrl) {
        if (!VALID_TABS.has(tab)) tab = 'overview';
        this.activeTab = tab;

        this.elements.tabs.forEach((button) => {
            const active = button.dataset.tabTarget === tab;
            button.classList.toggle('is-active', active);
            button.setAttribute('aria-selected', String(active));
            button.tabIndex = active ? 0 : -1;
        });
        this.elements.panels.forEach((panel) => {
            panel.hidden = panel.dataset.tabPanel !== tab;
        });

        this.clearRefreshTimer();
        if (tab === 'overview') this.scheduleRefresh();
        if (updateUrl) this.writeLocation(tab);
    }

    async loadOverview() {
        if (this.overviewController) this.overviewController.abort();
        const controller = new AbortController();
        this.overviewController = controller;
        this.setButtonBusy(this.elements.refresh, true, '刷新中');
        this.elements.overviewError.hidden = true;

        try {
            const payload = await this.fetchPayload(API.overview, controller.signal);
            if (!payload.data) throw new Error('接口没有返回总览数据');
            this.overviewData = payload.data;
            this.lastOverviewAt = Date.now();
            this.renderOverview(payload.data);
        } catch (error) {
            if (error.name === 'AbortError') return;
            this.renderOverviewError(error);
        } finally {
            if (this.overviewController === controller) {
                this.overviewController = null;
                this.setButtonBusy(this.elements.refresh, false, '刷新数据');
                this.scheduleRefresh();
            }
        }
    }

    renderOverview(data) {
        const service = data.service || {};
        this.setServiceState('online', '服务在线');
        setText('service-version', service.version || '—');
        setText('service-uptime', service.uptime_human ? '运行 ' + service.uptime_human : '—');
        setText('metric-latest-id', data.latest_data ? '#' + formatNumber(data.latest_data.save_id) : '暂无');
        setText('metric-web-requests', formatNumber(service.web_request_count));
        setText('metric-api-requests', formatNumber(service.api_request_count));
        setText('metric-min-id', formatNumber(service.min_lookup_id));
        setText('updated-at', '更新于 ' + formatClock(new Date()));

        if (Number.isInteger(Number(service.min_lookup_id))) {
            this.minLookupId = Number(service.min_lookup_id);
            this.elements.lookupInput.min = String(this.minLookupId);
            setText('lookup-min-id', formatNumber(this.minLookupId));
        }

        const records = [
            { label: '最近数据', type: data.latest_data?.save_type || 'data', value: data.latest_data },
            { label: '飞船', type: 'ship', value: data.latest_ship },
            { label: '存档', type: 'save', value: data.latest_save },
        ];
        this.elements.latestRecords.innerHTML = records.map(renderRecordRow).join('');
        this.elements.overviewError.hidden = true;
    }

    renderOverviewError(error) {
        this.setServiceState('error', '连接异常');
        this.elements.overviewError.textContent = this.overviewData
            ? '刷新失败，当前仍显示上次成功获取的数据。' + error.message
            : '暂时无法读取服务数据。' + error.message;
        this.elements.overviewError.hidden = false;
        if (!this.overviewData) {
            this.elements.latestRecords.innerHTML =
                '<tr class="table-empty"><td colspan="6">总览数据载入失败，请稍后重试。</td></tr>';
        }
    }

    setServiceState(state, label) {
        const element = document.getElementById('service-state');
        element.dataset.state = state;
        setText('service-state-label', label);
    }

    scheduleRefresh() {
        this.clearRefreshTimer();
        if (document.hidden || this.activeTab !== 'overview' || this.overviewController) return;
        const elapsed = Date.now() - this.lastOverviewAt;
        const delay = this.lastOverviewAt === 0
            ? REFRESH_INTERVAL_MS
            : Math.max(1000, REFRESH_INTERVAL_MS - elapsed);
        this.refreshTimer = window.setTimeout(() => this.loadOverview(), delay);
    }

    clearRefreshTimer() {
        if (this.refreshTimer !== null) {
            window.clearTimeout(this.refreshTimer);
            this.refreshTimer = null;
        }
    }

    openRecord(id) {
        if (!Number.isInteger(id)) return;
        this.setTab('record', false);
        this.elements.lookupInput.value = String(id);
        this.lookupRecord(id, { updateUrl: true });
    }

    async lookupRecord(providedId = null, options = { updateUrl: true }) {
        const id = providedId === null ? Number(this.elements.lookupInput.value) : Number(providedId);
        if (!Number.isInteger(id) || id < this.minLookupId) {
            this.elements.lookupInput.setAttribute('aria-invalid', 'true');
            this.elements.lookupFeedback.textContent = '请输入不小于 ' + formatNumber(this.minLookupId) + ' 的整数';
            this.elements.lookupInput.focus();
            return;
        }

        this.elements.lookupInput.removeAttribute('aria-invalid');
        this.elements.lookupFeedback.textContent = '';
        if (options.updateUrl !== false) this.writeLocation('record', id);

        if (this.lookupController) this.lookupController.abort();
        const controller = new AbortController();
        this.lookupController = controller;
        this.currentRecord = null;
        this.showQueryPlaceholder('正在读取 #' + id, '正在加载记录元数据和 XML 原文…', 'loading');
        this.setButtonBusy(this.elements.lookupSubmit, true, '查询中');

        try {
            const detailPayload = await this.fetchPayload(API.record(id), controller.signal);
            if (!detailPayload.data?.info) throw new Error('记录不存在');

            const detail = detailPayload.data;
            let rawData = typeof detail.raw_data === 'string' ? detail.raw_data : null;
            if (rawData === null) {
                try {
                    const rawPayload = await this.fetchPayload(API.raw(id), controller.signal);
                    rawData = typeof rawPayload.data?.raw_data === 'string' ? rawPayload.data.raw_data : '';
                } catch (error) {
                    if (error.name === 'AbortError') throw error;
                    rawData = '';
                }
            }

            this.currentRecord = {
                info: detail.info,
                xml_status: detail.xml_status,
                raw_data: rawData,
            };
            this.renderRecord(this.currentRecord);
        } catch (error) {
            if (error.name === 'AbortError') return;
            this.showQueryPlaceholder('查询失败', error.message || '无法读取这条记录，请稍后重试。', 'error');
        } finally {
            if (this.lookupController === controller) {
                this.lookupController = null;
                this.setButtonBusy(this.elements.lookupSubmit, false, '查询档案');
            }
        }
    }

    renderRecord(record) {
        const info = record.info;
        const hasRaw = Boolean(record.raw_data);
        this.xmlAnalysis = analyzeXml(record.raw_data);
        this.rawDisplayMode = this.xmlAnalysis.valid ? 'formatted' : 'raw';
        this.rawWrap = false;
        setText('record-title', '#' + info.save_id);
        setText('record-type', formatType(info.save_type) + ' · ' + formatBytes(info.len));
        setText('record-id-detail', formatNumber(info.save_id));
        setText('record-data-type', formatType(info.save_type));
        setText('record-length', formatBytes(info.len) + ' (' + formatNumber(info.len) + ' B)');
        setText('record-character-count', hasRaw ? formatNumber(record.raw_data.length) : '—');
        setText('record-xml-detail', formatXmlStatus(record.xml_status, info.xml_tested));
        setText('record-root-element', this.xmlAnalysis.root || '—');
        setText('record-element-count', this.xmlAnalysis.valid ? formatNumber(this.xmlAnalysis.elements) : '—');
        setText('record-attribute-count', this.xmlAnalysis.valid ? formatNumber(this.xmlAnalysis.attributes) : '—');
        setText('record-hash', info.blake_hash || '—');
        setText('raw-size', hasRaw ? formatBytes(new Blob([record.raw_data]).size) : '无原文');

        const xmlStatus = document.getElementById('record-xml-status');
        xmlStatus.dataset.state = info.xml_tested ? 'ok' : 'warn';
        xmlStatus.textContent = info.xml_tested ? 'XML 通过' : 'XML 异常';

        document.getElementById('download-record').disabled = !hasRaw;
        document.getElementById('copy-record-raw').disabled = !hasRaw;
        this.elements.formatToggle.disabled = !this.xmlAnalysis.valid;
        this.elements.wrapToggle.disabled = !hasRaw;
        this.renderRawView();
        this.elements.queryPlaceholder.hidden = true;
        this.elements.recordDetail.hidden = false;
    }

    renderRawView() {
        const hasRaw = Boolean(this.currentRecord?.raw_data);
        const canFormat = Boolean(this.xmlAnalysis?.valid);
        const formatted = hasRaw && canFormat && this.rawDisplayMode === 'formatted';
        const content = !hasRaw
            ? '该记录没有可用的原始内容。'
            : formatted
                ? this.xmlAnalysis.formatted
                : this.currentRecord.raw_data;

        setText('record-raw', content);
        setText('raw-view-mode', formatted
            ? '格式化视图 · ' + formatNumber(this.xmlAnalysis.lines) + ' 行'
            : '原始视图');
        this.elements.formatToggle.textContent = formatted ? '查看原文' : '格式化显示';
        this.elements.formatToggle.setAttribute('aria-pressed', String(formatted));
        this.elements.wrapToggle.textContent = this.rawWrap ? '关闭换行' : '自动换行';
        this.elements.wrapToggle.setAttribute('aria-pressed', String(this.rawWrap));
        this.elements.rawView.classList.toggle('is-wrapped', this.rawWrap);
    }

    resetQuery() {
        if (this.lookupController) this.lookupController.abort();
        this.currentRecord = null;
        this.xmlAnalysis = null;
        this.elements.lookupInput.value = '';
        this.elements.lookupFeedback.textContent = '';
        this.showQueryPlaceholder('等待查询', '输入一个记录 ID，即可读取归档详情。', 'idle');
    }

    showQueryPlaceholder(title, message, state) {
        setText('query-placeholder-title', title);
        setText('query-placeholder-message', message);
        this.elements.queryPlaceholder.dataset.state = state;
        this.elements.queryPlaceholder.hidden = false;
        this.elements.recordDetail.hidden = true;
    }

    async fetchPayload(url, signal) {
        const response = await fetch(url, {
            signal,
            headers: { Accept: 'application/json' },
        });
        let payload;
        try {
            payload = await response.json();
        } catch {
            throw new Error('服务返回了无法识别的响应');
        }

        if (!response.ok || !payload || Number(payload.code) >= 400) {
            throw new Error(payload?.msg || '请求失败 (' + response.status + ')');
        }
        return payload;
    }

    setButtonBusy(button, busy, label) {
        button.disabled = busy;
        button.setAttribute('aria-busy', String(busy));
        button.textContent = label;
    }

    async copyText(value, successMessage) {
        try {
            if (navigator.clipboard && window.isSecureContext) {
                await navigator.clipboard.writeText(value);
            } else {
                const textarea = document.createElement('textarea');
                textarea.value = value;
                textarea.setAttribute('readonly', '');
                textarea.style.position = 'fixed';
                textarea.style.opacity = '0';
                document.body.appendChild(textarea);
                textarea.select();
                const copied = document.execCommand('copy');
                textarea.remove();
                if (!copied) throw new Error('copy failed');
            }
            this.showToast(successMessage);
        } catch {
            this.showToast('复制失败，请手动选择内容');
        }
    }

    downloadRecord() {
        if (!this.currentRecord?.raw_data) return;
        const info = this.currentRecord.info;
        const safeType = String(info.save_type || 'data').replace(/[^a-z0-9_-]/gi, '-');
        const blob = new Blob([this.currentRecord.raw_data], { type: 'application/xml;charset=utf-8' });
        const objectUrl = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = objectUrl;
        link.download = 'sr-download-' + info.save_id + '-' + safeType + '.xml';
        document.body.appendChild(link);
        link.click();
        link.remove();
        window.setTimeout(() => URL.revokeObjectURL(objectUrl), 0);
        this.showToast('XML 下载已开始');
    }

    showToast(message) {
        window.clearTimeout(this.toastTimer);
        this.elements.toast.textContent = message;
        this.elements.toast.hidden = false;
        this.toastTimer = window.setTimeout(() => {
            this.elements.toast.hidden = true;
        }, 2400);
    }
}

function renderRecordRow(item) {
    const record = item.value;
    if (!record) {
        return '<tr>' +
            '<td data-label="类别">' + escapeHtml(item.label) + '</td>' +
            '<td data-label="记录 ID">暂无</td><td data-label="大小">—</td>' +
            '<td data-label="XML">—</td><td data-label="Blake3">—</td><td></td></tr>';
    }

    const id = Number(record.save_id);
    const hash = record.blake_hash || '';
    const xmlState = record.xml_tested ? 'ok' : 'warn';
    const xmlLabel = record.xml_tested ? '通过' : '异常';
    return '<tr>' +
        '<td data-label="类别"><strong>' + escapeHtml(item.label) + '</strong><br><small>' +
            escapeHtml(formatType(item.type)) + '</small></td>' +
        '<td data-label="记录 ID"><button class="record-id-button" type="button" data-record-id="' + id + '">#' +
            formatNumber(id) + '</button></td>' +
        '<td data-label="大小">' + escapeHtml(formatBytes(record.len)) + '</td>' +
        '<td data-label="XML"><span class="status-badge" data-state="' + xmlState + '">' + xmlLabel + '</span></td>' +
        '<td data-label="Blake3"><button class="text-button" type="button" data-copy-value="' +
            escapeHtml(hash) + '" title="复制完整哈希"><code>' + escapeHtml(truncateHash(hash)) + '</code></button></td>' +
        '<td><button class="button button--secondary row-action" type="button" data-record-id="' + id +
            '">查看详情</button></td></tr>';
}

function setText(id, value) {
    const element = document.getElementById(id);
    if (element) element.textContent = value;
}

function formatNumber(value) {
    const number = Number(value);
    return Number.isFinite(number) ? number.toLocaleString('zh-CN') : '—';
}

function formatBytes(value) {
    const bytes = Number(value);
    if (!Number.isFinite(bytes) || bytes < 0) return '—';
    if (bytes < 1024) return bytes + ' B';
    const units = ['KB', 'MB', 'GB', 'TB'];
    let size = bytes;
    let unitIndex = -1;
    do {
        size /= 1024;
        unitIndex += 1;
    } while (size >= 1024 && unitIndex < units.length - 1);
    return size.toFixed(size >= 10 ? 0 : 1) + ' ' + units[unitIndex];
}

function formatType(value) {
    const type = String(value || '').toLowerCase();
    if (type === 'ship') return '飞船';
    if (type === 'save') return '存档';
    if (type === 'data') return '数据';
    return value || '未知';
}

function formatXmlStatus(value, xmlTested) {
    const labels = {
        'verified ship': '飞船结构校验通过',
        'verified save': '存档 XML 校验通过',
        'valid xml': 'XML 格式有效',
        'not xml': 'XML 格式无效',
        'not ship': '不是有效的飞船结构',
        'fake ship': '飞船结构无法解析',
        'broken ship': '飞船结构不完整',
        'empty data': '没有数据',
    };
    return labels[value] || value || (xmlTested ? 'XML 校验通过' : 'XML 校验失败');
}

function analyzeXml(xml) {
    const empty = {
        valid: false,
        root: '',
        elements: 0,
        attributes: 0,
        lines: 0,
        formatted: '',
    };
    if (!xml) return empty;

    try {
        const documentNode = new DOMParser().parseFromString(xml, 'application/xml');
        if (documentNode.querySelector('parsererror')) return empty;
        const elements = Array.from(documentNode.getElementsByTagName('*'));
        const formatted = formatXml(xml);
        return {
            valid: true,
            root: documentNode.documentElement?.tagName || '',
            elements: elements.length,
            attributes: elements.reduce((count, element) => count + element.attributes.length, 0),
            lines: formatted ? formatted.split('\n').length : 0,
            formatted,
        };
    } catch {
        return empty;
    }
}

function formatXml(xml) {
    const lines = xml
        .replace(/>\s*</g, '><')
        .replace(/></g, '>\n<')
        .split('\n');
    const output = [];
    let depth = 0;

    for (const sourceLine of lines) {
        const line = sourceLine.trim();
        if (!line) continue;

        const closing = /^<\//.test(line);
        const declaration = /^<\?|^<!/.test(line);
        const selfClosing = /\/>$/.test(line);
        const closesOnSameLine = /<\/[^>]+>$/.test(line);
        if (closing) depth = Math.max(0, depth - 1);
        output.push('  '.repeat(depth) + line);

        const opening = /^<[^!?/][^>]*>$/.test(line);
        if (opening && !closing && !declaration && !selfClosing && !closesOnSameLine) {
            depth += 1;
        }
    }
    return output.join('\n');
}

function formatClock(date) {
    return new Intl.DateTimeFormat('zh-CN', {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false,
    }).format(date);
}

function truncateHash(hash) {
    if (!hash) return '—';
    return hash.length > 18 ? hash.slice(0, 9) + '…' + hash.slice(-7) : hash;
}

function escapeHtml(value) {
    return String(value)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;');
}

document.addEventListener('DOMContentLoaded', () => {
    new DashboardApp().init();
});
