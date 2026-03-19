'use strict';

const API = {
    overview: '/api/overview',
    service: '/api/service',
    record: (id) => `/api/records/${id}`,
    raw: (id) => `/api/records/${id}/raw`,
};

const MIN_LOOKUP_ID = 76858;

class SignalFeed {
    constructor(root) {
        this.root = root;
    }

    push(level, message) {
        if (!this.root) return;

        const empty = this.root.querySelector('.signal-feed__empty');
        if (empty) {
            empty.remove();
        }

        const line = document.createElement('p');
        line.className = 'signal-line';
        line.innerHTML = [
            `<span class="signal-line__time">${new Date().toLocaleTimeString('zh-CN', { hour12: false })}</span>`,
            `<span class="signal-line__level">${escapeHtml(level)}</span>`,
            `<span>${escapeHtml(message)}</span>`,
        ].join('');
        this.root.prepend(line);

        while (this.root.childElementCount > 14) {
            this.root.removeChild(this.root.lastElementChild);
        }
    }
}

class DashboardApp {
    constructor() {
        this.signalFeed = new SignalFeed(document.getElementById('signal-feed'));
        this.lookupForm = document.getElementById('lookup-form');
        this.lookupInput = document.getElementById('record-id');
        this.lookupResult = document.getElementById('lookup-result');
        this.refreshButton = document.getElementById('refresh-overview');
    }

    init() {
        this.bindEvents();
        this.loadOverview();
        this.loadServiceStatus();
    }

    bindEvents() {
        this.lookupForm?.addEventListener('submit', (event) => {
            event.preventDefault();
            this.lookupRecord();
        });

        this.refreshButton?.addEventListener('click', () => {
            this.loadOverview();
            this.loadServiceStatus();
        });
    }

    async loadOverview() {
        this.signalFeed.push('GET', API.overview);
        const response = await this.fetchJson(API.overview);
        if (!response) return;

        this.renderOverview(response.data);
        this.signalFeed.push('OK', 'overview updated');
    }

    async loadServiceStatus() {
        const response = await this.fetchJson(API.service, false);
        if (!response) return;
        this.renderService(response.data);
    }

    async lookupRecord() {
        const id = Number.parseInt(this.lookupInput?.value ?? '', 10);
        if (!Number.isInteger(id) || id < MIN_LOOKUP_ID) {
            this.renderLookupError(`ID 必须是不小于 ${MIN_LOOKUP_ID} 的整数`);
            this.signalFeed.push('WARN', `invalid id input: ${this.lookupInput?.value ?? ''}`);
            return;
        }

        this.lookupResult.className = 'result-card';
        this.lookupResult.innerHTML = '<p>正在加载记录详情...</p>';
        this.signalFeed.push('GET', API.record(id));

        const detail = await this.fetchJson(API.record(id));
        if (!detail) {
            this.renderLookupError('记录请求失败');
            return;
        }

        if (!detail.data) {
            this.renderLookupError(detail.msg || '记录不存在');
            return;
        }

        const raw = await this.fetchJson(API.raw(id), false);
        const rawData = raw?.data?.raw_data ?? detail.data.raw_data ?? '';
        this.renderLookupSuccess(detail.data, rawData);
        this.signalFeed.push('OK', `record ${id} loaded`);
    }

    async fetchJson(url, reportErrors = true) {
        try {
            const response = await fetch(url, {
                headers: {
                    Accept: 'application/json',
                },
            });
            const payload = await response.json();
            if (!response.ok || payload.code >= 400) {
                throw new Error(payload.msg || `request failed: ${response.status}`);
            }
            return payload;
        } catch (error) {
            if (reportErrors) {
                this.signalFeed.push('ERR', `${url} -> ${error.message}`);
            }
            return null;
        }
    }

    renderOverview(data) {
        if (!data) return;
        this.renderRecordCard('data', data.latest_data);
        this.renderRecordCard('ship', data.latest_ship);
        this.renderRecordCard('save', data.latest_save);
        this.renderService(data.service);
    }

    renderService(service) {
        if (!service) return;
        setText('service-version', service.version);
        setText('service-uptime', `${service.uptime_human} (${service.uptime_seconds}s)`);
        setText('web-requests', formatNumber(service.web_request_count));
        setText('api-requests', formatNumber(service.api_request_count));
        setText('min-lookup-id', formatNumber(service.min_lookup_id));
    }

    renderRecordCard(prefix, record) {
        if (!record) {
            setText(`latest-${prefix}-id`, 'N/A');
            setText(`latest-${prefix}-len`, '-');
            setText(`latest-${prefix}-xml`, '-');
            setText(`latest-${prefix}-hash`, '-');
            if (prefix === 'data') {
                setText('latest-data-type', 'no record');
            }
            return;
        }

        setText(`latest-${prefix}-id`, `#${record.save_id}`);
        setText(`latest-${prefix}-len`, formatBytes(record.len));
        setText(`latest-${prefix}-xml`, record.xml_tested ? 'passed' : 'failed');
        setText(`latest-${prefix}-hash`, truncateHash(record.blake_hash));
        if (prefix === 'data') {
            setText('latest-data-type', record.save_type);
        }
    }

    renderLookupSuccess(detail, rawData) {
        const xmlState = detail.info.xml_tested ? 'ok' : 'warn';
        const preview = rawData ? rawData.slice(0, 2400) : 'No raw data available';

        this.lookupResult.className = 'result-card';
        this.lookupResult.innerHTML = `
            <div class="result-header">
                <div>
                    <h3>#${detail.info.save_id}</h3>
                    <p>${escapeHtml(detail.info.save_type)} | ${formatBytes(detail.info.len)}</p>
                </div>
                <span class="status-pill" data-state="${xmlState}">${escapeHtml(detail.xml_status)}</span>
            </div>
            <div class="result-grid">
                <section class="result-block">
                    <h4>Metadata</h4>
                    <pre>${escapeHtml(JSON.stringify(detail.info, null, 2))}</pre>
                </section>
                <section class="result-block">
                    <h4>Raw Preview</h4>
                    <pre>${escapeHtml(preview)}</pre>
                </section>
            </div>
        `;
    }

    renderLookupError(message) {
        this.lookupResult.className = 'result-card';
        this.lookupResult.innerHTML = `
            <div class="result-header">
                <div>
                    <h3>Lookup Failed</h3>
                    <p>${escapeHtml(message)}</p>
                </div>
            </div>
        `;
    }
}

function setText(id, value) {
    const element = document.getElementById(id);
    if (element) {
        element.textContent = value;
    }
}

function escapeHtml(value) {
    return String(value)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;');
}

function formatNumber(value) {
    return Number(value).toLocaleString('zh-CN');
}

function formatBytes(value) {
    const bytes = Number(value);
    if (!Number.isFinite(bytes)) return '-';
    if (bytes < 1024) return `${bytes} B`;

    const units = ['KB', 'MB', 'GB', 'TB'];
    let size = bytes;
    let unit = -1;
    while (size >= 1024 && unit < units.length - 1) {
        size /= 1024;
        unit += 1;
    }
    return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unit]}`;
}

function truncateHash(hash) {
    if (!hash) return '-';
    return hash.length > 20 ? `${hash.slice(0, 10)}...${hash.slice(-8)}` : hash;
}

document.addEventListener('DOMContentLoaded', () => {
    new DashboardApp().init();
});
