'use strict';

const API = Object.freeze({
    overview: '/api/overview',
    market: '/api/market',
    record: (id) => '/api/records/' + id,
    analysis: (id) => '/api/records/' + id + '/analysis',
    raw: (id) => '/api/records/' + id + '/raw',
});

const DEFAULT_MIN_ID = 76858;
const REFRESH_INTERVAL_MS = 30000;
const VALID_TABS = new Set(['overview', 'market', 'record']);

class DashboardApp {
    constructor() {
        this.activeTab = 'overview';
        this.minLookupId = DEFAULT_MIN_ID;
        this.lastOverviewAt = 0;
        this.overviewData = null;
        this.currentRecord = null;
        this.xmlAnalysis = null;
        this.rawDisplayMode = 'formatted';
        this.xmlSplit = true;
        this.rawWrap = false;
        this.refreshTimer = null;
        this.overviewController = null;
        this.marketController = null;
        this.marketRecords = [];
        this.marketBefore = null;
        this.marketHasMore = false;
        this.marketLoaded = false;
        this.marketFilter = 'all';
        this.marketLimit = 48;
        this.lookupController = null;
        this.toastTimer = null;

        this.elements = {
            tabs: Array.from(document.querySelectorAll('[data-tab-target]')),
            panels: Array.from(document.querySelectorAll('[data-tab-panel]')),
            refresh: document.getElementById('refresh-overview'),
            overviewError: document.getElementById('overview-error'),
            latestRecords: document.getElementById('latest-records'),
            marketRefresh: document.getElementById('refresh-market'),
            marketType: document.getElementById('market-type'),
            marketLimit: document.getElementById('market-limit'),
            marketUpdatedAt: document.getElementById('market-updated-at'),
            marketCount: document.getElementById('market-count'),
            marketError: document.getElementById('market-error'),
            marketGrid: document.getElementById('market-grid'),
            marketLoadMore: document.getElementById('market-load-more'),
            lookupForm: document.getElementById('lookup-form'),
            lookupInput: document.getElementById('record-id'),
            lookupSubmit: document.getElementById('lookup-submit'),
            lookupFeedback: document.getElementById('lookup-feedback'),
            queryPlaceholder: document.getElementById('query-placeholder'),
            recordDetail: document.getElementById('record-detail'),
            rawView: document.getElementById('record-raw-view'),
            xmlHighlightView: document.getElementById('xml-highlight-view'),
            sectionToggle: document.getElementById('toggle-xml-sections'),
            shipAnalysis: document.getElementById('ship-analysis'),
            shipAnalysisError: document.getElementById('ship-analysis-error'),
            shipAnalysisMetrics: document.getElementById('ship-analysis-metrics'),
            shipInventory: document.getElementById('ship-inventory'),
            shipFuel: document.getElementById('ship-fuel'),
            shipStaging: document.getElementById('ship-staging'),
            shipStructure: document.getElementById('ship-structure'),
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
        this.elements.marketRefresh.addEventListener('click', () => this.loadMarket(true));
        this.elements.marketType.addEventListener('change', () => {
            this.marketFilter = this.elements.marketType.value;
            this.loadMarket(true);
        });
        this.elements.marketLimit.addEventListener('change', () => {
            this.marketLimit = Number(this.elements.marketLimit.value) || 48;
            this.loadMarket(true);
        });
        this.elements.marketLoadMore.addEventListener('click', () => this.loadMarket(false));
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
        this.elements.marketGrid.addEventListener('click', (event) => {
            const recordButton = event.target.closest('[data-record-id]');
            if (recordButton) this.openRecord(Number(recordButton.dataset.recordId));
            const copyButton = event.target.closest('[data-copy-value]');
            if (copyButton) this.copyText(copyButton.dataset.copyValue, '哈希已复制');
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
        this.elements.sectionToggle.addEventListener('click', () => {
            if (!this.xmlAnalysis?.valid) return;
            this.xmlSplit = !this.xmlSplit;
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
        if (tab === 'market' && !this.marketLoaded && !this.marketController) this.loadMarket(true);
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

    async loadMarket(reset = true) {
        if (this.marketController) this.marketController.abort();
        if (reset) {
            this.marketBefore = null;
            this.marketRecords = [];
            this.marketHasMore = false;
            this.marketLoaded = false;
            this.elements.marketGrid.innerHTML = '<div class="market-empty">正在载入最近数据…</div>';
        }
        const controller = new AbortController();
        this.marketController = controller;
        this.elements.marketError.hidden = true;
        this.setButtonBusy(this.elements.marketRefresh, true, '刷新中');
        if (!reset) this.setButtonBusy(this.elements.marketLoadMore, true, '载入中');

        try {
            const params = new URLSearchParams({
                limit: String(this.marketLimit),
                type: this.marketFilter,
            });
            if (!reset && this.marketBefore !== null) {
                params.set('before', String(this.marketBefore));
            }
            const payload = await this.fetchPayload(API.market + '?' + params.toString(), controller.signal);
            const page = payload.data || {};
            const records = Array.isArray(page.records) ? page.records : [];
            this.marketRecords = reset ? records : this.marketRecords.concat(records);
            this.marketBefore = page.next_before ?? null;
            this.marketHasMore = Boolean(page.has_more);
            this.marketLoaded = true;
            this.renderMarket();
        } catch (error) {
            if (error.name === 'AbortError') return;
            this.elements.marketError.textContent = '市场数据载入失败：' + (error.message || '未知错误');
            this.elements.marketError.hidden = false;
            if (reset) {
                this.marketRecords = [];
                this.renderMarket();
            }
        } finally {
            if (this.marketController === controller) {
                this.marketController = null;
                this.setButtonBusy(this.elements.marketRefresh, false, '刷新市场');
                if (!reset) this.setButtonBusy(this.elements.marketLoadMore, false, '加载更早的数据');
            }
        }
    }

    renderMarket() {
        this.elements.marketGrid.innerHTML = this.marketRecords.length
            ? this.marketRecords.map(renderMarketCard).join('')
            : '<div class="market-empty"><strong>这里还没有数据</strong><span>当前筛选条件下没有可展示的记录。</span></div>';
        this.elements.marketLoadMore.hidden = !this.marketHasMore;
        this.elements.marketCount.textContent = this.marketRecords.length
            ? '已显示 ' + formatNumber(this.marketRecords.length) + ' 条'
            : '暂无记录';
        this.elements.marketUpdatedAt.textContent = '更新于 ' + formatClock(new Date());
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

            let shipAnalysis = null;
            let shipAnalysisError = '';
            if (isVerifiedShip(detail.info, detail.xml_status)) {
                try {
                    const analysisPayload = await this.fetchPayload(API.analysis(id), controller.signal);
                    shipAnalysis = analysisPayload.data || null;
                } catch (error) {
                    if (error.name === 'AbortError') throw error;
                    shipAnalysisError = error.message || '后端分析接口暂时不可用';
                }
            }

            this.currentRecord = {
                info: detail.info,
                xml_status: detail.xml_status,
                raw_data: rawData,
                ship_analysis: shipAnalysis,
                ship_analysis_error: shipAnalysisError,
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
        this.xmlSplit = true;
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
        renderXmlOutline(this.xmlAnalysis);

        const xmlStatus = document.getElementById('record-xml-status');
        xmlStatus.dataset.state = info.xml_tested ? 'ok' : 'warn';
        xmlStatus.textContent = info.xml_tested ? 'XML 通过' : 'XML 异常';

        document.getElementById('download-record').disabled = !hasRaw;
        document.getElementById('copy-record-raw').disabled = !hasRaw;
        this.elements.formatToggle.disabled = !this.xmlAnalysis.valid;
        this.elements.sectionToggle.disabled = !this.xmlAnalysis.valid;
        this.elements.wrapToggle.disabled = !hasRaw;
        this.renderShipAnalysis(record);
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

        const useInspector = formatted && this.xmlSplit;
        if (formatted) {
            this.elements.xmlHighlightView.innerHTML = useInspector
                ? renderXmlInspector(this.xmlAnalysis.formatted, this.xmlAnalysis.outline)
                : renderXmlContinuous(this.xmlAnalysis.formatted);
            this.elements.xmlHighlightView.hidden = false;
            this.elements.rawView.hidden = true;
        } else {
            setText('record-raw', content);
            this.elements.xmlHighlightView.hidden = true;
            this.elements.rawView.hidden = false;
        }
        setText('raw-view-mode', formatted
            ? (useInspector ? '分区高亮视图 · ' : '连续高亮视图 · ') +
                formatNumber(this.xmlAnalysis.lines) + ' 行'
            : '原始视图');
        this.elements.formatToggle.textContent = formatted ? '查看原文' : '格式化显示';
        this.elements.formatToggle.setAttribute('aria-pressed', String(formatted));
        this.elements.sectionToggle.textContent = this.xmlSplit ? '连续显示' : '分区显示';
        this.elements.sectionToggle.setAttribute('aria-pressed', String(this.xmlSplit));
        this.elements.wrapToggle.textContent = this.rawWrap ? '关闭换行' : '自动换行';
        this.elements.wrapToggle.setAttribute('aria-pressed', String(this.rawWrap));
        this.elements.rawView.classList.toggle('is-wrapped', this.rawWrap);
        this.elements.xmlHighlightView.classList.toggle('is-wrapped', this.rawWrap);
    }

    renderShipAnalysis(record) {
        const eligible = isVerifiedShip(record.info, record.xml_status);
        const analysis = record.ship_analysis;
        this.elements.shipAnalysis.hidden = !eligible;
        this.elements.shipAnalysisError.hidden = !record.ship_analysis_error;
        this.elements.shipAnalysisError.textContent = record.ship_analysis_error || '';
        if (!eligible || !analysis) {
            this.elements.shipAnalysisMetrics.innerHTML = '';
            this.elements.shipInventory.innerHTML = '';
            this.elements.shipFuel.innerHTML = '';
            this.elements.shipStaging.innerHTML = '';
            this.elements.shipStructure.innerHTML = '';
            return;
        }

        const totals = analysis.totals || {};
        const mass = analysis.mass || {};
        const fuel = analysis.fuel || {};
        const propulsion = analysis.propulsion || {};
        const metrics = [
            ['部件', formatNumber(totals.parts)],
            ['估算质量', formatScaledNumber(mass.scaled_units)],
            ['当前燃料', formatScaledNumber(fuel.current)],
            ['引擎推力', formatScaledNumber(propulsion.engines?.power)],
            ['连接', formatNumber(analysis.connections?.total)],
            ['级序', formatNumber(analysis.staging?.length)],
        ];
        this.elements.shipAnalysisMetrics.innerHTML = metrics.map(([label, value]) =>
            '<div class="analysis-metric"><span class="analysis-metric__label">' +
            escapeHtml(label) + '</span><strong class="analysis-metric__value">' +
            escapeHtml(value) + '</strong></div>'
        ).join('');

        this.elements.shipInventory.innerHTML = (analysis.inventory || []).length
            ? analysis.inventory.map(renderInventoryRow).join('')
            : '<tr><td colspan="6" class="table-empty">没有部件记录</td></tr>';

        const fuelBuckets = fuel.by_type || [];
        this.elements.shipFuel.innerHTML = [
            '<div class="analysis-list__item"><div class="analysis-list__title"><span>总燃料</span><code>' +
                escapeHtml(formatScaledNumber(fuel.current)) + ' / ' + escapeHtml(formatScaledNumber(fuel.capacity)) +
                '</code></div><div class="analysis-list__meta">容量使用率 ' +
                escapeHtml(formatPercent(fuel.current, fuel.capacity)) + '</div></div>',
            ...fuelBuckets.map((bucket) =>
                '<div class="analysis-list__item"><div class="analysis-list__title"><span>' +
                escapeHtml(bucket.fuel_type || '未知燃料') + '</span><code>' +
                escapeHtml(formatScaledNumber(bucket.current)) + ' / ' +
                escapeHtml(formatScaledNumber(bucket.capacity)) +
                '</code></div></div>'
            ),
            '<div class="analysis-list__item"><div class="analysis-list__title"><span>引擎</span><code>' +
                escapeHtml(formatNumber(propulsion.engines?.count)) + ' 个 · ' +
                escapeHtml(formatScaledNumber(propulsion.engines?.consumption)) + ' /s</code></div>' +
                '<div class="analysis-list__meta">RCS ' +
                escapeHtml(formatNumber(propulsion.rcs?.count)) + ' 个 · 太阳能板 ' +
                escapeHtml(formatNumber(propulsion.solar_count)) + ' 个</div></div>',
        ].join('');
        setText('ship-propulsion-summary',
            formatScaledNumber(propulsion.engines?.power) + ' 推力');

        this.elements.shipStaging.innerHTML = (analysis.staging || []).length
            ? analysis.staging.map(renderPodAnalysis).join('')
            : '<div class="analysis-list__empty">没有 Pod 级序信息</div>';
        setText('ship-staging-summary', formatNumber(analysis.staging?.length) + ' 个 Pod');

        const geometry = analysis.geometry;
        const connection = analysis.connections || {};
        const structureItems = [
            '<div class="analysis-list__item"><div class="analysis-list__title"><span>飞船状态</span><code>' +
                escapeHtml(analysis.state?.lifted_off ? '已起飞' : '未起飞') + ' · ' +
                escapeHtml(analysis.state?.touching_ground ? '接地' : '空中') +
                '</code></div><div class="analysis-list__meta">版本 ' +
                escapeHtml(String(analysis.state?.version ?? '—')) + '</div></div>',
            '<div class="analysis-list__item"><div class="analysis-list__title"><span>连接</span><code>' +
                escapeHtml(formatNumber(connection.total)) + ' 条</code></div><div class="analysis-list__meta">普通 ' +
                escapeHtml(formatNumber(connection.normal)) + ' · Dock ' +
                escapeHtml(formatNumber(connection.dock)) + ' · 断开组 ' +
                escapeHtml(formatNumber(analysis.totals?.disconnected_groups)) + '</div></div>',
            '<div class="analysis-list__item"><div class="analysis-list__title"><span>布局包围盒</span><code>' +
                escapeHtml(geometry ? formatScaledNumber(geometry.width) + ' × ' + formatScaledNumber(geometry.height) : '不可估算') +
                '</code></div><div class="analysis-list__meta">' +
                escapeHtml(geometry ? '已解析 ' + formatNumber(geometry.known_parts) + ' 个部件' : '部件目录缺少尺寸') +
                '</div></div>',
            '<div class="analysis-list__item"><div class="analysis-list__title"><span>活动部件</span><code>' +
                escapeHtml(formatNumber(analysis.totals?.active_parts)) + ' / ' +
                escapeHtml(formatNumber(analysis.totals?.parts)) + '</code></div><div class="analysis-list__meta">爆炸 ' +
                escapeHtml(formatNumber(analysis.totals?.exploded_parts)) + ' · 未知类型 ' +
                escapeHtml(formatNumber(analysis.totals?.unknown_part_types)) + '</div></div>',
        ];
        this.elements.shipStructure.innerHTML = structureItems.join('');
        setText('ship-geometry-summary', geometry ? '目录尺寸估算' : '无目录尺寸');
    }

    resetQuery() {
        if (this.lookupController) this.lookupController.abort();
        this.currentRecord = null;
        this.xmlAnalysis = null;
        this.elements.lookupInput.value = '';
        this.elements.lookupFeedback.textContent = '';
        this.elements.shipAnalysis.hidden = true;
        this.elements.xmlHighlightView.hidden = true;
        this.elements.rawView.hidden = false;
        renderXmlOutline(null);
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

function renderMarketCard(record) {
    const id = Number(record.save_id);
    const xmlState = record.xml_tested ? 'ok' : 'warn';
    const xmlLabel = record.xml_tested ? 'XML 通过' : 'XML 未通过';
    const hash = record.blake_hash || '';
    return '<article class="market-card">' +
        '<header class="market-card__header"><span class="type-pill type-pill--' +
        escapeHtml(String(record.save_type || 'unknown').toLowerCase()) + '">' +
        escapeHtml(formatType(record.save_type)) + '</span><span class="market-card__time">' +
        escapeHtml(formatRecordTime(record.recorded_at)) + '</span></header>' +
        '<div class="market-card__identity"><button class="record-id-button" type="button" data-record-id="' + id + '">#' +
        formatNumber(id) + '</button><span class="status-badge" data-state="' + xmlState + '">' + xmlLabel + '</span></div>' +
        '<dl class="market-card__meta"><div><dt>大小</dt><dd>' + escapeHtml(formatBytes(record.len)) + '</dd></div>' +
        '<div><dt>数据类型</dt><dd>' + escapeHtml(formatType(record.save_type)) + '</dd></div></dl>' +
        '<div class="market-card__hash"><code title="复制完整哈希">' + escapeHtml(truncateHash(hash)) + '</code>' +
        '<button class="text-button" type="button" data-copy-value="' + escapeHtml(hash) + '">复制</button></div>' +
        '<button class="button button--secondary market-card__action" type="button" data-record-id="' + id + '">查看档案</button>' +
        '</article>';
}

function formatRecordTime(value) {
    const date = new Date(value);
    if (!Number.isFinite(date.getTime())) return '时间未知';
    return new Intl.DateTimeFormat('zh-CN', {
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
    }).format(date);
}

function isVerifiedShip(info, xmlStatus) {
    return String(info?.save_type || '').toLowerCase() === 'ship' &&
        Boolean(info?.xml_tested) &&
        String(xmlStatus || '').toLowerCase() === 'verified ship';
}

function formatScaledNumber(value) {
    const number = Number(value);
    return Number.isFinite(number)
        ? number.toLocaleString('zh-CN', { maximumFractionDigits: 2 })
        : '—';
}

function formatPercent(value, total) {
    const current = Number(value);
    const capacity = Number(total);
    if (!Number.isFinite(current) || !Number.isFinite(capacity) || capacity <= 0) return '—';
    return (current * 100 / capacity).toLocaleString('zh-CN', { maximumFractionDigits: 1 }) + '%';
}

function formatPartCategory(value) {
    const labels = {
        pod: '控制舱',
        tank: '燃料箱',
        engine: '引擎',
        rcs: 'RCS',
        detacher: '分离器',
        parachute: '降落伞',
        solar: '太阳能板',
        wheel: '轮组',
        fuselage: '机身',
        strut: '支架',
        nosecone: '整流罩',
        dockconnector: '对接插头',
        dockport: '对接口',
        lander: '着陆腿',
        Satellite: '卫星部件',
        unknown: '未知',
    };
    return labels[value] || value || '未知';
}

function renderInventoryRow(item) {
    const totalMass = Number(item.catalog_mass) * Number(item.count);
    const mass = Number.isFinite(totalMass) ? formatScaledNumber(totalMass) : '未知';
    const fuel = Number(item.fuel_capacity) > 0
        ? formatScaledNumber(item.current_fuel) + ' / ' + formatScaledNumber(item.fuel_capacity)
        : '—';
    const power = Number(item.engine_power) > 0
        ? formatScaledNumber(item.engine_power) + ' 推力'
        : Number(item.rcs_power) > 0
            ? formatScaledNumber(item.rcs_power) + ' RCS'
            : '—';
    return '<tr><td><strong>' + escapeHtml(item.name || '未知部件') + '</strong><br><code>' +
        escapeHtml(item.part_type_id || '—') + '</code></td><td>' +
        escapeHtml(formatPartCategory(item.category)) + '</td><td>' +
        escapeHtml(formatNumber(item.count)) + '</td><td><code>' + escapeHtml(mass) +
        '</code></td><td><code>' + escapeHtml(fuel) + '</code></td><td><code>' +
        escapeHtml(power) + '</code></td></tr>';
}

function renderPodAnalysis(pod) {
    const steps = (pod.steps || []).map((step) => {
        const activations = (step.activations || []).map((activation) => {
            const label = activation.part_name
                ? activation.part_name + ' #' + activation.part_id
                : '#' + activation.part_id;
            return '<span class="status-badge" data-state="' + (activation.moved ? 'ok' : 'neutral') +
                '">' + escapeHtml(label) + '</span>';
        }).join(' ');
        return '<div class="analysis-list__meta">阶段 ' + escapeHtml(String(step.index)) +
            '：' + (activations || '无触发部件') + '</div>';
    }).join('');
    return '<div class="analysis-list__item"><div class="analysis-list__title"><span>' +
        escapeHtml(pod.name || '未命名 Pod') + ' #' + escapeHtml(String(pod.part_id)) +
        '</span><code>阶段 ' + escapeHtml(String(pod.current_stage ?? '—')) +
        ' · 油门 ' + escapeHtml(formatPercent(pod.throttle, 1)) +
        '</code></div>' + (steps || '<div class="analysis-list__meta">没有级序步骤</div>') + '</div>';
}

function renderXmlInspector(formatted, outline = null) {
    const lines = String(formatted || '').split('\n');
    const blocks = [];
    let rootLines = [];
    let active = null;
    let activeDepth = 0;

    for (const line of lines) {
        const trimmed = line.trim();
        const depth = Math.max(0, Math.floor((line.length - line.trimStart().length) / 2));
        const opening = trimmed.match(/^<([A-Za-z_][\w:.-]*)(?:\s|>|\/)/);
        if (!active && depth === 1 && opening) {
            if (rootLines.length) {
                blocks.push({ type: 'root', lines: rootLines });
                rootLines = [];
            }
            active = { name: opening[1], lines: [line], depth };
            activeDepth = depth;
            if (/\/\s*>$/.test(trimmed)) {
                blocks.push({ type: 'section', ...active });
                active = null;
            }
            continue;
        }
        if (active) {
            active.lines.push(line);
            if (depth === activeDepth && new RegExp('^</' + escapeRegExp(active.name) + '>').test(trimmed)) {
                blocks.push({ type: 'section', ...active });
                active = null;
            }
            continue;
        }
        rootLines.push(line);
    }
    if (active) blocks.push({ type: 'section', ...active });
    if (rootLines.length) blocks.push({ type: 'root', lines: rootLines });

    return blocks.map((block) => block.type === 'root'
        ? '<pre class="xml-highlight-view__root">' + highlightXml(block.lines.join('\n')) + '</pre>'
        : '<details open class="xml-section-block xml-section-block--' +
            escapeHtml(String(block.name).toLowerCase().replace(/[^a-z0-9_-]/g, '-')) + '"><summary>' +
            escapeHtml(formatXmlSectionName(block.name, outline)) +
        '</summary><pre class="xml-highlight-view__code">' +
        highlightXml(block.lines.join('\n')) + '</pre></details>'
    ).join('');
}

function formatXmlSectionName(name, outline) {
    const labels = {
        Parts: '部件 · Parts',
        Connections: '连接 · Connections',
        DisconnectedParts: '断开部件 · DisconnectedParts',
        Nodes: '场景节点 · Nodes',
        Ship: '飞船 · Ship',
    };
    const section = outline?.sections?.find((item) => item.name === name);
    const count = section?.children;
    return (labels[name] || name) + (Number.isFinite(count) ? ' · ' + formatNumber(count) + ' 个子节点' : '');
}

function escapeRegExp(value) {
    return String(value).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function renderXmlContinuous(formatted) {
    return '<pre class="xml-highlight-view__root">' +
        highlightXml(formatted) + '</pre>';
}

function highlightXml(xml) {
    return String(xml || '').split('\n').map((line) =>
        '<span class="xml-line">' + highlightXmlLine(line) + '</span>'
    ).join('');
}

function highlightXmlLine(line) {
    const tagPattern = /<[^>]*>/g;
    let output = '';
    let cursor = 0;
    let match;
    while ((match = tagPattern.exec(line)) !== null) {
        output += highlightXmlText(line.slice(cursor, match.index));
        output += highlightXmlTag(match[0]);
        cursor = match.index + match[0].length;
    }
    output += highlightXmlText(line.slice(cursor));
    return output;
}

function highlightXmlText(value) {
    if (!value) return '';
    return '<span class="xml-text">' + escapeHtml(value) + '</span>';
}

function highlightXmlTag(tag) {
    if (/^<!--/.test(tag) || /^<!\[CDATA/.test(tag)) {
        return '<span class="xml-comment">' + escapeHtml(tag) + '</span>';
    }
    const match = tag.match(/^(<\??\/?)([A-Za-z_][\w:.-]*)([\s\S]*?)(\/?>)$/);
    if (!match) return escapeHtml(tag);
    let output = escapeHtml(match[1]) + '<span class="xml-tag">' +
        escapeHtml(match[2]) + '</span>';
    const attributes = match[3];
    let cursor = 0;
    const attributePattern = /([A-Za-z_:][\w:.-]*)(\s*=\s*)(".*?"|'.*?')/g;
    let attribute;
    while ((attribute = attributePattern.exec(attributes)) !== null) {
        output += escapeHtml(attributes.slice(cursor, attribute.index));
        output += '<span class="xml-attr-name">' + escapeHtml(attribute[1]) + '</span>' +
            escapeHtml(attribute[2]) + '<span class="xml-attr-value">' +
            escapeHtml(attribute[3]) + '</span>';
        cursor = attribute.index + attribute[0].length;
    }
    output += escapeHtml(attributes.slice(cursor)) + escapeHtml(match[4]);
    return output;
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
        outline: null,
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
            outline: summarizeXmlNode(documentNode.documentElement),
        };
    } catch {
        return empty;
    }
}

function summarizeXmlNode(element) {
    if (!element) return null;
    const children = Array.from(element.children);
    const childCounts = new Map();
    for (const child of children) {
        childCounts.set(child.tagName, (childCounts.get(child.tagName) || 0) + 1);
    }
    const sections = children.map(summarizeXmlNode);
    return {
        name: element.tagName,
        attributes: element.attributes.length,
        elements: sections.reduce((total, section) => total + section.elements, 1),
        children: children.length,
        sections,
        attributeValues: Array.from(element.attributes, (attribute) => ({
            name: attribute.name,
            value: attribute.value,
        })),
        childCounts: Array.from(childCounts, ([name, count]) => ({ name, count })),
    };
}

function renderXmlOutline(analysis) {
    const metrics = document.getElementById('xml-outline-metrics');
    const facts = document.getElementById('xml-outline-facts');
    const sections = document.getElementById('xml-outline-sections');
    if (!metrics || !facts || !sections) return;
    if (!analysis?.valid || !analysis.outline) {
        metrics.innerHTML = '';
        facts.innerHTML = '';
        sections.innerHTML = '<div class="xml-outline__empty">无法解析 XML 结构。</div>';
        return;
    }
    const outline = analysis.outline;
    metrics.innerHTML = [
        ['根节点', outline.name],
        ['文档节点', formatNumber(analysis.elements)],
        ['属性', formatNumber(analysis.attributes)],
        ['一级分块', formatNumber(outline.children)],
    ].map(([label, value]) => '<div class="xml-outline-metric"><span>' + escapeHtml(label) +
        '</span><strong>' + escapeHtml(value) + '</strong></div>').join('');
    facts.innerHTML = outline.attributeValues?.length
        ? '<span class="xml-outline-facts__label">根节点属性</span>' + outline.attributeValues.map((attribute) =>
            '<span class="xml-outline-fact"><code>' + escapeHtml(attribute.name) +
            '</code><span>' + escapeHtml(attribute.value) + '</span></span>').join('')
        : '';
    sections.innerHTML = outline.sections.length
        ? outline.sections.map(renderXmlOutlineSection).join('')
        : '<div class="xml-outline__empty">根节点没有子节点。</div>';
}

function renderXmlOutlineSection(section) {
    const classes = 'xml-outline-section xml-outline-section--' +
        String(section.name).toLowerCase().replace(/[^a-z0-9_-]/g, '-');
    const children = section.childCounts?.length
        ? section.childCounts.map((child) => '<span class="xml-outline-chip"><code>' +
            escapeHtml(child.name) + '</code> × ' + formatNumber(child.count) + '</span>').join('')
        : '<span class="xml-outline-muted">没有子节点</span>';
    return '<article class="' + classes + '"><header><strong>' +
        escapeHtml(formatXmlSectionName(section.name, { sections: [section] })) +
        '</strong><span>' + formatNumber(section.elements) + ' 个节点 · ' +
        formatNumber(section.attributes) + ' 个属性</span></header><div class="xml-outline-chips">' +
        children + '</div></article>';
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
