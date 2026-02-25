// Global State & UI References
const els = {
    sidebarLeft: document.getElementById('panelA'),
    sidebarRight: document.getElementById('rightPanel'),
    panelB: document.getElementById('panelB'),

    statusDot: document.getElementById('connectionStatus'),
    statusText: document.getElementById('statusText'),
    chartTitle: document.getElementById('chartTitle'),
    balance: document.getElementById('balanceAmount'),
    serverTime: document.getElementById('serverTime'),
    stats: {
        total: document.getElementById('totalTrades'),
        wins: document.getElementById('totalWins'),
        losses: document.getElementById('totalLosses'),
        profit: document.getElementById('profitLoss'),
        streak: document.getElementById('winStreak')
    },
    log: document.getElementById('tradeLog')
};

let ws;
let chart, candleSeries, shortEmaSeries, longEmaSeries;
let candleData = [];
let winCount = 0;
let lossCount = 0;
let totalTrades = 0;
let winStreak = 0;
let runningProfit = 0.0;
let activeTrades = new Map();
let emaConfig = { shortPeriod: 9, longPeriod: 21, shortType: 'EMA', longType: 'EMA' };

// --- Sidebar Logic ---
document.getElementById('sidebarTriggerLeft').addEventListener('mouseenter', () => {
    els.sidebarLeft.classList.add('active');
    els.panelB.classList.add('left-open');
});
document.getElementById('closeSidebar').addEventListener('click', () => {
    els.sidebarLeft.classList.remove('active');
    els.panelB.classList.remove('left-open');
});
document.getElementById('menuToggle').addEventListener('click', () => {
    els.sidebarLeft.classList.toggle('active');
    els.panelB.classList.toggle('left-open');
});
document.getElementById('sidebarTriggerRight').addEventListener('mouseenter', () => {
    // Only open if toggle is checked
    const toggle = document.getElementById('rightSidebarToggle');
    if (toggle && toggle.checked) {
        els.sidebarRight.classList.add('active');
        els.panelB.classList.add('right-open');
    }
});
document.getElementById('closeRightSidebar').addEventListener('click', () => {
    els.sidebarRight.classList.remove('active');
    els.panelB.classList.remove('right-open');
});

document.getElementById('fullscreenBtn').addEventListener('click', () => {
    if (!document.fullscreenElement) {
        document.documentElement.requestFullscreen();
    } else {
        document.exitFullscreen();
    }
});

// --- Trading Mode Logic ---
const modeButtons = document.querySelectorAll('.btn-mode');
modeButtons.forEach(btn => {
    btn.addEventListener('click', () => {
        modeButtons.forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        updateTradeMode(btn.dataset.mode);
    });
});

function updateTradeMode(mode) {
    // UI Sync if called externally
    modeButtons.forEach(b => {
        if (b.dataset.mode === mode) {
            b.classList.add('active');
        } else {
            b.classList.remove('active');
        }
    });

    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({
            command: "UPDATE_MODE",
            trade_mode: mode
        }));
    }
}

// --- Chart Init ---
function initChart() {
    const container = document.getElementById('chartContainer');
    chart = LightweightCharts.createChart(container, {
        width: container.clientWidth,
        height: 500,
        layout: {
            background: { color: '#0a0e27' },
            textColor: '#d1d4dc',
        },
        grid: {
            vertLines: { color: '#1a1f3a' },
            horzLines: { color: '#1a1f3a' },
        },
        rightPriceScale: { borderColor: '#2a3f5f' },
        timeScale: {
            borderColor: '#2a3f5f',
            timeVisible: true,
            secondsVisible: false,
        },
    });

    candleSeries = chart.addCandlestickSeries({
        upColor: '#4caf50',
        downColor: '#f44336',
        borderUpColor: '#4caf50',
        borderDownColor: '#f44336',
        wickUpColor: '#4caf50',
        wickDownColor: '#f44336',
    });

    shortEmaSeries = chart.addLineSeries({ color: '#00BFFF', lineWidth: 2, title: 'Short EMA' });
    longEmaSeries = chart.addLineSeries({ color: '#FF6347', lineWidth: 2, title: 'Long EMA' });

    window.addEventListener('resize', () => {
        chart.applyOptions({ width: container.clientWidth });
    });
}

// --- Connection & Trading ---
document.getElementById('clearLogBtn').addEventListener('click', () => {
    els.log.innerHTML = '<div class="log-empty">No trades yet.</div>';
});

function connectWS() {
    els.statusText.textContent = "Connecting...";

    if (!ws || ws.readyState !== WebSocket.OPEN) {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        ws = new WebSocket(protocol + '//' + window.location.host + '/ws');

        ws.onopen = () => {
            updateStatus(true);
            requestData();
        };
        ws.onmessage = handleMessage;
        ws.onclose = () => updateStatus(false);
        ws.onerror = () => updateStatus(false);
    } else {
        requestData();
    }
}

function disconnectWS() {
    if (ws) {
        ws.close();
    }
}

function updateStatus(connected) {
    if (connected) {
        els.statusDot.classList.add('connected');
        els.statusText.textContent = "Connected";
    } else {
        els.statusDot.classList.remove('connected');
        els.statusText.textContent = "Disconnected";
    }
}

function disconnectAndUnsubscribe() {
    if (ws && ws.readyState === WebSocket.OPEN) {
        // Update status immediately
        els.statusText.textContent = "Stopping streams...";

        // Send stop command
        ws.send(JSON.stringify({ command: "STOP_STREAMS" }));
        console.log("📤 Sent STOP_STREAMS command");

        // Wait a bit longer for server to process, then close
        setTimeout(() => {
            if (ws && ws.readyState === WebSocket.OPEN) {
                ws.close();
                console.log("🔌 WebSocket closed after STOP_STREAMS");
            }

            // Reset UI states
            els.statusDot.classList.remove('connected');
            els.statusText.textContent = "Disconnected (Unsubscribed)";

            // Clear chart data
            candleData = [];
            if (candleSeries) candleSeries.setData([]);
            if (shortEmaSeries) shortEmaSeries.setData([]);
            if (longEmaSeries) longEmaSeries.setData([]);

            // Reset active trades
            activeTrades.clear();
            updateActiveTradesTable();

            // Show notification
            showDisconnectNotification();
        }, 1000);
    } else {
        console.log("⚠️ WebSocket not connected");
        els.statusText.textContent = "Not connected";
    }
}

function showDisconnectNotification() {
    const notification = document.createElement('div');
    notification.style.cssText = `
        position: fixed;
        bottom: 20px;
        right: 20px;
        padding: 15px 25px;
        background: linear-gradient(135deg, #ff9900 0%, #e67700 100%);
        color: white;
        border-radius: 10px;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
        z-index: 9999;
        font-weight: 500;
    `;
    notification.innerHTML = '🔌 Disconnected & Unsubscribed successfully!';
    document.body.appendChild(notification);

    setTimeout(() => notification.remove(), 3000);
}

function requestData() {
    // Reset Chart Data
    candleData = [];
    candleSeries.setData([]);
    shortEmaSeries.setData([]);
    longEmaSeries.setData([]);
    activeTrades.clear();

    const asset = document.getElementById('assetSelect').value;
    const tradeMode = document.querySelector('.btn-mode.active').dataset.mode;
    const moneyMode = document.getElementById('moneyMode').value;
    const initialStake = parseFloat(document.getElementById('initialStake').value) || 1.0;
    const apiToken = document.getElementById('apiToken').value.trim();
    const appId = document.getElementById('appId').value.trim();
    const duration = parseInt(document.getElementById('duration').value);
    const durationUnit = document.getElementById('durationUnit').value;
    const targetProfit = parseFloat(document.getElementById('targetMoney').value);
    const targetWin = parseInt(document.getElementById('numWinTarget').value);

    const msg = JSON.stringify({
        command: "START_DERIV",
        asset, trade_mode: tradeMode, money_mode: moneyMode,
        initial_stake: initialStake, api_token: apiToken, app_id: appId,
        duration, duration_unit: durationUnit,
        target_profit: targetProfit, target_win: targetWin
    });
    ws.send(msg);
}

function updateParams() {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const moneyMode = document.getElementById('moneyMode').value;
    const duration = parseInt(document.getElementById('duration').value);
    const durationUnit = document.getElementById('durationUnit').value;
    const targetProfit = parseFloat(document.getElementById('targetMoney').value);
    const targetWin = parseInt(document.getElementById('numWinTarget').value);

    ws.send(JSON.stringify({
        command: "UPDATE_PARAMS",
        money_mode: moneyMode,
        duration, duration_unit: durationUnit,
        target_profit: targetProfit, target_win: targetWin
    }));
}

// --- Message Handling ---
function handleMessage(event) {
    const data = JSON.parse(event.data);

    if (data.type === "server_time") {
        const date = new Date(data.server_time * 1000);
        els.serverTime.textContent = date.toLocaleTimeString();
    } else if (data.type === "balance") {
        els.balance.textContent = "$" + data.balance.toFixed(2);
    } else if (data.type === "ema_data") {
        updateEma(data);
    } else if (data.symbol) {
        updateCandles(data);
    } else if (data.type === "analysis_data" || data.msg_type === "analysis_data" || (data.ema_short_slope_direction && data.action)) {
        // Handle Analysis Data
        handleAnalysisData(data);
    } else if (data.type === "trade_opened" || data.msg_type === "trade_opened") {
        handleTradeOpened(data);
    } else if (data.type === "trade_update" || data.msg_type === "trade_update") {
        handleTradeUpdate(data);
    } else if (data.type === "trade_result") {
        handleTradeResult(data);
    } else if (data.type === "lot_status") {
        // handle lot status if needed for stats
        // here we use lot status to auto-switch to idle if stopped
        if (!data.lot_active) {
            updateTradeMode('idle');
        }
    }
}

function updateCandles(data) {
    const minute = Math.floor(data.time / 60) * 60;
    const existingIndex = candleData.findIndex(c => c.time === minute);

    if (existingIndex >= 0) {
        candleData[existingIndex].high = Math.max(candleData[existingIndex].high, data.high);
        candleData[existingIndex].low = Math.min(candleData[existingIndex].low, data.low);
        candleData[existingIndex].close = data.close;
    } else {
        candleData.push({
            time: minute,
            open: data.open, high: data.high, low: data.low, close: data.close
        });
    }

    // Keep only last 200
    if (candleData.length > 200) candleData = candleData.slice(-200);

    candleData.sort((a, b) => a.time - b.time);
    candleSeries.setData(candleData);

    // Update Title with price
    els.chartTitle.textContent = `${data.symbol}: ${data.close.toFixed(4)}`;
}

function updateEma(data) {
    if (data.short_ema && data.short_ema.length > 0) {
        shortEmaSeries.setData(data.short_ema.filter(p => p.value > 0).sort((a, b) => a.time - b.time));
    }
    if (data.long_ema && data.long_ema.length > 0) {
        longEmaSeries.setData(data.long_ema.filter(p => p.value > 0).sort((a, b) => a.time - b.time));
    }
}

function handleAnalysisData(data) {
    const tbody = document.getElementById('analysis-data-body');

    // Log for debugging
    // console.log("Analysis Data:", data);

    const slopeColor = data.ema_short_slope_direction === 'Up' ? '#00ff00' :
        data.ema_short_slope_direction === 'Down' ? '#ff4444' : '#888';

    const turnColor = data.is_ema_short_turn_type === 'TurnUp' ? '#00ff00' :
        data.is_ema_short_turn_type === 'TurnDown' ? '#ff4444' : '#888';

    const diffColor = data.ema_diff > 0 ? '#00ff00' : data.ema_diff < 0 ? '#ff4444' : '#888';

    const actionColor = data.action === 'Call' ? '#2ecc71' :
        data.action === 'Put' ? '#e74c3c' : '#f1c40f';

    const emaShortVal = (data.ema_short_value !== undefined && data.ema_short_value !== null) ? data.ema_short_value.toFixed(4) : '-';
    const emaDiffVal = (data.ema_diff !== undefined && data.ema_diff !== null) ? data.ema_diff.toFixed(4) : '-';

    const row = `
        <tr style="border-bottom: 1px solid #333;">
            <td style="padding: 10px; color: ${slopeColor}; font-weight: bold;">${data.ema_short_slope_direction || '-'}</td>
            <td style="padding: 10px;">${emaShortVal}</td>
            <td style="padding: 10px; color: ${turnColor};">${data.is_ema_short_turn_type || '-'}</td>
            <td style="padding: 10px; color: ${diffColor};">${emaDiffVal}</td>
            <td style="padding: 10px; color: ${actionColor}; font-weight: bold;">${data.action || '-'}</td>
        </tr>
    `;

    // Replace content to show current state (or prepend if history desired, but request was 'update consistently')
    // User asked for "update every 2 seconds", implying a status board. 
    // We will just show the latest row to keep it clean, or maybe a small history. 
    // Let's keep it defined as "Latest Status" for now based on 'index_backup' style which had single value fields.
    // But since I made a table, I'll clear and set the new row.

    tbody.innerHTML = row;
}

function handleTradeOpened(data) {
    console.log("📝 Trade Opened:", data);
    // Create initial trade object
    activeTrades.set(data.contract_id, {
        contract_id: data.contract_id,
        asset: data.asset,
        trade_type: data.trade_type,
        buy_price: data.stake, // Initially stake is buy price
        payout: 0,
        profit: 0,
        profit_percentage: 0,
        date_start: data.time, // Unix timestamp
        date_expiry: 0, // Will update later
        min_profit: 0,
        max_profit: 0
    });
    updateActiveTradesTable();
}

function handleTradeUpdate(data) {
    // console.log("📊 Trade Update:", data);
    let currentTrade = activeTrades.get(data.contract_id) || {};

    // Update min/max profit
    let profit = data.profit || 0;
    let minProfit = currentTrade.min_profit !== undefined ? Math.min(currentTrade.min_profit, profit) : profit;
    let maxProfit = currentTrade.max_profit !== undefined ? Math.max(currentTrade.max_profit, profit) : profit;

    activeTrades.set(data.contract_id, {
        ...currentTrade,
        contract_id: data.contract_id,
        entry_spot: data.entry_spot,
        current_spot: data.current_spot,
        buy_price: data.buy_price || currentTrade.buy_price,
        payout: data.payout || 0,
        profit: profit,
        profit_percentage: data.profit_percentage || 0,
        date_expiry: data.date_expiry || currentTrade.date_expiry,
        date_start: data.date_start || currentTrade.date_start,
        is_sold: data.is_sold,
        is_expired: data.is_expired,
        asset: data.asset || currentTrade.asset || '-',
        trade_type: data.trade_type || currentTrade.trade_type || '-',
        min_profit: minProfit,
        max_profit: maxProfit
    });

    updateActiveTradesTable();
}

function updateActiveTradesTable() {
    const tbody = document.getElementById('active-trades-body');

    if (activeTrades.size === 0) {
        tbody.innerHTML = '<tr><td colspan="13" class="log-empty">No active trades. Trades will appear here when opened.</td></tr>';
        return;
    }

    const rows = Array.from(activeTrades.values()).map(function (trade, index) {
        const now = Math.floor(Date.now() / 1000);
        const timeLeft = (trade.date_expiry || 0) - now;
        const timeLeftStr = trade.date_expiry ? formatDuration(timeLeft) : '-';
        const timeClass = timeLeft <= 10 && timeLeft > -100 ? 'time-warning' : '';

        const profitClass = trade.profit >= 0 ? 'text-success' : 'text-danger'; // Using existing utility classes if any, or inline style
        const profitColor = trade.profit >= 0 ? '#00ff00' : '#ff4444';
        const profitSign = trade.profit >= 0 ? '+' : '';

        const typeBadge = trade.trade_type === 'CALL'
            ? '<span style="background: rgba(0,255,0,0.2); color: #00ff00; padding: 2px 5px; border-radius: 4px;">CALL</span>'
            : '<span style="background: rgba(255,68,68,0.2); color: #ff4444; padding: 2px 5px; border-radius: 4px;">PUT</span>';

        // Profit details
        const profitDisplay = `<span style="color: ${profitColor}; font-weight: bold;">${profitSign}${trade.profit.toFixed(2)}</span> <span style="font-size: 11px; color: #aaa;">[${trade.profit_percentage ? trade.profit_percentage.toFixed(2) : '0.00'}%]</span>`;

        return `
            <tr style="border-bottom: 1px solid #333;">
                <td style="padding: 10px;">${index + 1}</td>
                <td style="padding: 10px; font-family: monospace; font-size: 12px; color: #888;">${trade.contract_id}</td>
                <td style="padding: 10px;"><strong>${trade.asset}</strong></td>
                <td style="padding: 10px;">${typeBadge}</td>
                <td style="padding: 10px;">$${(trade.buy_price ? Number(trade.buy_price).toFixed(2) : '0.00')}</td>
                <td style="padding: 10px;">$${(trade.payout ? Number(trade.payout).toFixed(2) : '0.00')}</td>
                <td style="padding: 10px;">${profitDisplay}</td>
                <td style="padding: 10px;">${formatTime(trade.date_start)}</td>
                <td style="padding: 10px;">${formatTime(trade.date_expiry)}</td>
                <td style="padding: 10px; font-family: monospace;" class="${timeClass}">${timeLeftStr}</td>
                <td style="padding: 10px; color: #ff4444;">${(trade.min_profit !== undefined ? trade.min_profit.toFixed(2) : '-')}</td>
                <td style="padding: 10px; color: #00ff00;">${(trade.max_profit !== undefined ? trade.max_profit.toFixed(2) : '-')}</td>
                <td style="padding: 10px;"><button onclick="sellContract('${trade.contract_id}')" class="btn btn-sm btn-danger" style="padding: 2px 8px; font-size: 11px;">SELL</button></td>
            </tr>
        `;
    });

    tbody.innerHTML = rows.join('');
}

function formatTime(unixTime) {
    if (!unixTime) return '-';
    const date = new Date(unixTime * 1000);
    return date.toTimeString().split(' ')[0];
}

function formatDuration(seconds) {
    if (seconds < 0) return '00:00:00';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    return [h, m, s].map(v => v < 10 ? "0" + v : v).join(":");
}

function sellContract(contractId) {
    if (ws && ws.readyState === WebSocket.OPEN) {
        if (confirm("Are you sure you want to SELL this contract?")) {
            console.log("🔻 Selling contract:", contractId);
            ws.send(JSON.stringify({
                command: "SELL",
                contract_id: contractId
            }));
        }
    } else {
        alert("WebSocket not connected");
    }
}

function handleTradeResult(data) {
    // ... existing code ...
    // Also remove from active trades map
    if (data.contract_id) {
        activeTrades.delete(data.contract_id);
        updateActiveTradesTable();
    }

    totalTrades++;
    // ... rest of handleTradeResult ...
    if (data.status === 'win') {
        winCount++;
        winStreak++;
        runningProfit += data.profit;
    } else {
        lossCount++;
        winStreak = 0;
        runningProfit += data.profit;
    }

    els.stats.total.textContent = totalTrades;
    els.stats.wins.textContent = winCount;
    els.stats.losses.textContent = lossCount;
    els.stats.streak.textContent = winStreak;

    els.stats.profit.textContent = "$" + runningProfit.toFixed(2);
    els.stats.profit.className = "stat-value " + (runningProfit >= 0 ? "text-success" : "text-danger"); // Use text-success/danger if defined or keep existing logic

    // Log History
    const logEntry = document.createElement('div');
    logEntry.className = `log-entry ${data.status}`;
    logEntry.innerHTML = `
        <div>${new Date().toLocaleTimeString()}</div>
        <div>${data.status.toUpperCase()}</div>
        <div>stake: $${data.stake}</div>
        <div>profit: $${data.profit.toFixed(2)}</div>
        <div>bal: $${data.balance.toFixed(2)}</div>
     `;
    if (els.log.querySelector('.log-empty')) els.log.innerHTML = '';
    els.log.insertBefore(logEntry, els.log.firstChild);
}

function updateClock() {
    const now = new Date();
    const hours = now.getHours();
    const minutes = now.getMinutes();
    const seconds = now.getSeconds();

    const hourAngle = (hours % 12) * 30 + minutes * 0.5;
    const minuteAngle = minutes * 6 + seconds * 0.1;
    const secondAngle = seconds * 6;

    const hourHand = document.getElementById('hourHand');
    const minuteHand = document.getElementById('minuteHand');
    const secondHand = document.getElementById('secondHand');

    if (hourHand) hourHand.style.transform = 'rotate(' + hourAngle + 'deg)';
    if (minuteHand) minuteHand.style.transform = 'rotate(' + minuteAngle + 'deg)';
    if (secondHand) secondHand.style.transform = 'rotate(' + secondAngle + 'deg)';

    const digitalTime = document.getElementById('digitalTime');
    if (digitalTime) {
        const h = hours.toString().padStart(2, '0');
        const m = minutes.toString().padStart(2, '0');
        const s = seconds.toString().padStart(2, '0');
        digitalTime.innerText = `${h}:${m}:${s}`;
    }
}

// Apply config values to UI
function applyConfigToUI(config) {
    console.log('📡 Applying config to UI:', config);

    // Update Asset Select with assets from config
    const assetSelect = document.getElementById('assetSelect');
    if (assetSelect && config.assetList) {
        // Preserve optgroups structure or create simple list
        const volatilityAssets = config.assetList.filter(a => a.symbol.startsWith('R_') || a.symbol.includes('HZ'));

        // Clear existing options in Volatility optgroup
        const volatilityGroup = assetSelect.querySelector('optgroup[label="Volatility Indices"]');
        if (volatilityGroup) {
            volatilityGroup.innerHTML = '';
            volatilityAssets.forEach(asset => {
                const option = document.createElement('option');
                option.value = asset.symbol;
                option.textContent = asset.name;
                if (asset.symbol === config.defaultAsset) {
                    option.selected = true;
                }
                volatilityGroup.appendChild(option);
            });
        }
    }

    // Update Initial Stake
    const initialStakeInput = document.getElementById('initialStake');
    if (initialStakeInput && config.startMoneyTrade) {
        initialStakeInput.value = config.startMoneyTrade;
    }

    // Update Target Money
    const targetMoneyInput = document.getElementById('targetMoney');
    if (targetMoneyInput && config.targetMoney) {
        targetMoneyInput.value = config.targetMoney;
    }

    // Update Money Mode based on tradeType
    const moneyModeSelect = document.getElementById('moneyMode');
    if (moneyModeSelect && config.selectedTradeType) {
        moneyModeSelect.value = config.selectedTradeType === 'MartinGaleTrade' ? 'martingale' : 'fix';
    }
}

// Setup config change listener
function setupConfigListener() {
    if (typeof configManager !== 'undefined') {
        // Apply initial config
        const currentConfig = configManager.getConfig();
        applyConfigToUI(currentConfig);

        // Listen for changes from other pages
        configManager.onConfigChange((config) => {
            console.log('📡 Config updated from setup page!');
            applyConfigToUI(config);

            // Show notification
            showConfigUpdateNotification();
        });
    }
}
// ==================== CHOPPY METER FUNCTIONS ====================

let meterEnabled = true;
let meterScanInterval = null;
let meterData = [];
const METER_SCAN_INTERVAL_MS = 60000; // 1 minute

// Toggle meter section visibility
function toggleMeterSection() {
    const section = document.getElementById('meterSection');
    const toggle = document.getElementById('meterToggle');

    meterEnabled = !meterEnabled;

    if (meterEnabled) {
        section.classList.remove('collapsed');
        toggle.classList.add('active');
        toggle.textContent = '📊 ON';
        startMeterAutoScan();
    } else {
        section.classList.add('collapsed');
        toggle.classList.remove('active');
        toggle.textContent = '📊 OFF';
        stopMeterAutoScan();
    }
}

// Start auto scanning
function startMeterAutoScan() {
    if (meterScanInterval) clearInterval(meterScanInterval);
    meterScanInterval = setInterval(() => {
        if (meterEnabled) {
            scanAllAssets();
        }
    }, METER_SCAN_INTERVAL_MS);
}

// Stop auto scanning
function stopMeterAutoScan() {
    if (meterScanInterval) {
        clearInterval(meterScanInterval);
        meterScanInterval = null;
    }
}

// Scan all assets for CI and ADX
async function scanAllAssets() {
    const statusEl = document.getElementById('meterStatus');
    const gridEl = document.getElementById('meterGrid');

    statusEl.textContent = 'Scanning...';
    statusEl.classList.add('scanning');

    // Get assets from config
    const config = typeof configManager !== 'undefined' ? configManager.getConfig() : null;
    const assets = config?.assetList || [
        { symbol: 'R_10', name: 'Vol 10' },
        { symbol: 'R_25', name: 'Vol 25' },
        { symbol: 'R_50', name: 'Vol 50' },
        { symbol: 'R_75', name: 'Vol 75' },
        { symbol: 'R_100', name: 'Vol 100' }
    ];

    document.getElementById('meterAssetCount').textContent = `(${assets.length} assets)`;

    // Show loading
    gridEl.innerHTML = `<div class="meter-loading"><div class="spinner"></div><p>Scanning ${assets.length} assets...</p></div>`;

    const results = [];

    for (const asset of assets) {
        try {
            const data = await fetchAssetIndicators(asset.symbol);
            if (data) {
                results.push({
                    symbol: asset.symbol,
                    name: asset.name,
                    ...data
                });
            }
        } catch (e) {
            console.error(`Error scanning ${asset.symbol}:`, e);
        }
    }

    // Sort by score (CI < 38.2 is best, ADX > 25 is best)
    results.sort((a, b) => {
        const scoreA = calculateScore(a.ci, a.adx);
        const scoreB = calculateScore(b.ci, b.adx);
        return scoreB - scoreA;
    });

    // Add rank
    results.forEach((r, i) => {
        r.rank = i + 1;
        r.score = calculateScore(r.ci, r.adx);
    });

    meterData = results;
    renderMeterGrid(results);

    statusEl.textContent = `Last: ${new Date().toLocaleTimeString()}`;
    statusEl.classList.remove('scanning');
}

// Calculate score (0-100, higher is better for trending)
function calculateScore(ci, adx) {
    // CI: lower is better (trending) - 0-38.2 is good
    // ADX: higher is better (trending) - > 25 is strong trend
    const ciScore = Math.max(0, 100 - (ci * 100 / 61.8));
    const adxScore = Math.min(100, (adx / 50) * 100);
    return (ciScore * 0.6) + (adxScore * 0.4);
}

// Fetch indicators for an asset using Deriv API
async function fetchAssetIndicators(symbol) {
    return new Promise((resolve) => {
        const wsUrl = 'wss://ws.derivws.com/websockets/v3?app_id=1089';
        const tempWs = new WebSocket(wsUrl);

        let candles = [];
        let timeout = setTimeout(() => {
            tempWs.close();
            resolve(null);
        }, 10000);

        tempWs.onopen = () => {
            tempWs.send(JSON.stringify({
                ticks_history: symbol,
                style: 'candles',
                granularity: 60,
                count: 50,
                end: 'latest'
            }));
        };

        tempWs.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);

                if (data.candles) {
                    candles = data.candles;

                    // Calculate CI and ADX
                    const ci = calculateCI(candles, 14);
                    const adx = calculateADX(candles, 14);
                    const recentCandles = candles.slice(-10).map(c => c.close > c.open ? 'up' : 'down');
                    const price = candles[candles.length - 1]?.close || 0;

                    clearTimeout(timeout);
                    tempWs.close();

                    resolve({
                        ci: ci,
                        adx: adx,
                        price: price,
                        recentCandles: recentCandles
                    });
                }

                if (data.error) {
                    clearTimeout(timeout);
                    tempWs.close();
                    resolve(null);
                }
            } catch (e) {
                console.error('Parse error:', e);
            }
        };

        tempWs.onerror = () => {
            clearTimeout(timeout);
            resolve(null);
        };
    });
}

// Calculate Choppiness Index
function calculateCI(candles, period = 14) {
    if (candles.length < period + 1) return 50;

    const slice = candles.slice(-period - 1);
    let atrSum = 0;
    let highestHigh = -Infinity;
    let lowestLow = Infinity;

    for (let i = 1; i < slice.length; i++) {
        const high = slice[i].high;
        const low = slice[i].low;
        const prevClose = slice[i - 1].close;

        // True Range
        const tr = Math.max(
            high - low,
            Math.abs(high - prevClose),
            Math.abs(low - prevClose)
        );
        atrSum += tr;

        if (high > highestHigh) highestHigh = high;
        if (low < lowestLow) lowestLow = low;
    }

    const range = highestHigh - lowestLow;
    if (range === 0) return 50;

    const ci = 100 * Math.log10(atrSum / range) / Math.log10(period);
    return Math.min(100, Math.max(0, ci));
}

// Calculate ADX (simplified)
function calculateADX(candles, period = 14) {
    if (candles.length < period + 1) return 25;

    const slice = candles.slice(-period - 1);
    let plusDMSum = 0;
    let minusDMSum = 0;
    let trSum = 0;

    for (let i = 1; i < slice.length; i++) {
        const high = slice[i].high;
        const low = slice[i].low;
        const prevHigh = slice[i - 1].high;
        const prevLow = slice[i - 1].low;
        const prevClose = slice[i - 1].close;

        const plusDM = Math.max(0, high - prevHigh);
        const minusDM = Math.max(0, prevLow - low);

        if (plusDM > minusDM) {
            plusDMSum += plusDM;
        } else {
            minusDMSum += minusDM;
        }

        const tr = Math.max(
            high - low,
            Math.abs(high - prevClose),
            Math.abs(low - prevClose)
        );
        trSum += tr;
    }

    if (trSum === 0) return 25;

    const plusDI = (plusDMSum / trSum) * 100;
    const minusDI = (minusDMSum / trSum) * 100;
    const diSum = plusDI + minusDI;

    if (diSum === 0) return 25;

    const dx = Math.abs(plusDI - minusDI) / diSum * 100;
    return Math.min(100, dx);
}

// Render meter grid
function renderMeterGrid(results) {
    const gridEl = document.getElementById('meterGrid');
    const currentAsset = document.getElementById('assetSelect')?.value || '';

    if (results.length === 0) {
        gridEl.innerHTML = '<div class="meter-loading"><p>No data available. Click "Scan Now" to scan.</p></div>';
        return;
    }

    gridEl.innerHTML = results.map((data, index) => {
        const ciClass = data.ci < 38.2 ? 'good' : data.ci > 61.8 ? 'bad' : 'neutral';
        const adxClass = data.adx > 25 ? 'good' : data.adx < 20 ? 'bad' : 'neutral';

        // Rank badge class
        let rankBadgeClass = '';
        if (data.rank === 1) rankBadgeClass = 'gold';
        else if (data.rank === 2) rankBadgeClass = 'silver';
        else if (data.rank === 3) rankBadgeClass = 'bronze';

        // Needle rotation (CI 0-100 maps to -90 to 90 degrees)
        const needleAngle = ((data.ci / 100) * 180) - 90;

        // Candles HTML
        const candlesHtml = (data.recentCandles || []).slice(-8).map(dir =>
            `<div class="candle-dot-mini ${dir}"></div>`
        ).join('');

        const isSelected = data.symbol === currentAsset;

        return `
            <div class="meter-card ${isSelected ? 'selected' : ''}" onclick="selectMeterAsset('${data.symbol}')" style="position: relative;">
                ${data.rank <= 3 ? `<span class="rank-badge ${rankBadgeClass}">#${data.rank}</span>` : `<span style="position: absolute; top: 5px; right: 8px; font-size: 10px; color: #757575;">#${data.rank}</span>`}
                <div class="asset-name">${data.symbol}</div>
                
                <!-- Gauge Meter -->
                <div class="gauge-container">
                    <svg viewBox="0 0 100 60">
                        <!-- Background arc -->
                        <path class="gauge-bg" d="M 10 50 A 40 40 0 0 1 90 50" />
                        
                        <!-- Value arc -->
                        <path class="gauge-value ${ciClass}" 
                              d="M 10 50 A 40 40 0 0 1 90 50"
                              stroke-dasharray="${125.6 * (data.ci / 100)} 125.6" />
                        
                        <!-- Needle -->
                        <g class="gauge-needle" style="transform: rotate(${needleAngle}deg)">
                            <line x1="50" y1="50" x2="50" y2="15" stroke="#fff" stroke-width="2" stroke-linecap="round"/>
                            <circle cx="50" cy="50" r="4" fill="#fff"/>
                        </g>
                        
                        <!-- CI Value -->
                        <text class="gauge-text" x="50" y="58">${data.ci.toFixed(1)}</text>
                    </svg>
                </div>
                
                <div class="metrics-row">
                    <div class="metric-item">
                        <div class="label">CI</div>
                        <div class="value ${ciClass}">${data.ci.toFixed(1)}</div>
                    </div>
                    <div class="metric-item">
                        <div class="label">ADX</div>
                        <div class="value ${adxClass}">${data.adx.toFixed(1)}</div>
                    </div>
                    <div class="metric-item">
                        <div class="label">Score</div>
                        <div class="value" style="color: #4fc3f7;">${data.score.toFixed(0)}</div>
                    </div>
                </div>
                
                <div class="candle-strip-mini">${candlesHtml}</div>
            </div>
        `;
    }).join('');
}

// Select asset from meter and load in chart
function selectMeterAsset(symbol) {
    console.log('🎯 Selected asset from meter:', symbol);

    // Update asset select
    const assetSelect = document.getElementById('assetSelect');
    if (assetSelect) {
        assetSelect.value = symbol;
    }

    // Update UI highlight
    document.querySelectorAll('.meter-card').forEach(card => {
        card.classList.remove('selected');
    });
    event.currentTarget.classList.add('selected');

    // If connected, switch asset
    if (ws && ws.readyState === WebSocket.OPEN) {
        // Reconnect with new asset
        connectWS();
    }
}

// ==================== END CHOPPY METER FUNCTIONS ====================

// Show notification when config is updated
function showConfigUpdateNotification() {
    const notification = document.createElement('div');
    notification.style.cssText = `
        position: fixed;
        bottom: 20px;
        right: 20px;
        padding: 15px 25px;
        background: linear-gradient(135deg, #00d9a0 0%, #00a085 100%);
        color: white;
        border-radius: 10px;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
        z-index: 9999;
        font-weight: 500;
        animation: slideIn 0.3s ease;
    `;
    notification.innerHTML = '✅ Config updated from Setup page!';
    document.body.appendChild(notification);

    setTimeout(() => {
        notification.style.animation = 'slideOut 0.3s ease';
        setTimeout(() => notification.remove(), 300);
    }, 3000);
}

window.onload = function () {
    // Check pktoken in URL
    const urlParams = new URLSearchParams(window.location.search);
    const pkToken = urlParams.get('pktoken');
    if (pkToken) {
        console.log("PK Token found:", pkToken);
        const apiTokenInput = document.getElementById('apiToken');
        if (apiTokenInput) {
            apiTokenInput.value = pkToken;
        }
    }

    initChart();
    setInterval(updateClock, 1000);
    updateClock();

    // Setup config listener after page load
    setTimeout(setupConfigListener, 100);

    // Start Choppy Meter auto scan after short delay
    setTimeout(() => {
        if (meterEnabled) {
            scanAllAssets();
            startMeterAutoScan();
        }
    }, 500);
};
