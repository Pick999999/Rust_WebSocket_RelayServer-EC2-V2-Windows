function calculateAllIndicators() {
    if (!candleData || candleData.length === 0) return;

    // MA Calculation
    for (let i = 1; i <= 3; i++) {
        const enabled = document.getElementById(`ma${i}Enabled`)?.checked;
        const period = parseInt(document.getElementById(`ma${i}Period`)?.value) || 20;
        const type = document.getElementById(`ma${i}Type`)?.value || 'EMA';
        const seriesObj = emaSeries[i - 1];

        if (enabled && seriesObj) {
            let data = [];
            if (type === 'EMA') data = calculateEMA(candleData, period);
            else if (type === 'HMA') data = calculateHMA(candleData, period);
            else if (type === 'EHMA') data = calculateEHMA(candleData, period);
            seriesObj.series.setData(data);
            currentMaData[i - 1] = data;
            //console.log('MAData',currentMaData);

        } else if (seriesObj) {
            seriesObj.series.setData([]);
            currentMaData[i - 1] = [];
        }
    }

    console.log('MAData', currentMaData);
    for (let i = 0; i <= currentMaData[0].length - 1; i++) {
        ema1 = currentMaData[0][i].value;
        ema2 = currentMaData[1][i].value;
        diff12 = Math.abs(ema2 - ema1).toFixed(4);
        if (diff12 < 0.18) {
            sMark = 'y';
        } else {
            sMark = 'n';
        }
        sObj = {
            time: currentMaData[0][i].time,
            macd: diff12,
            sMark: sMark
        }
        macd12.push(sObj);

    }
    console.log('macd12', macd12);


    const bb = calculateBB(candleData, 20);
    bbUpperSeries.setData(bb.upper); bbMiddleSeries.setData(bb.middle); bbLowerSeries.setData(bb.lower);

    currentCiValues = calculateCI(candleData, 14);
    ciSeries.setData(currentCiValues);

    currentAdxValues = calculateADX(candleData, 14);
    adxSeries.setData(currentAdxValues);

    // Calculate ATR with time for tooltip lookup
    currentAtrValues = calculateATRWithTime(candleData, 14);
    console.log('currentAtrValues', currentAtrValues);


    allMarkers = [];
    const thresh = parseFloat(document.getElementById('ciThreshold')?.value) || 61.8;
    currentCiValues.forEach(d => {
        if (d.value > thresh) allMarkers.push({ time: d.time, position: 'aboveBar', color: '#ef5350', shape: 'circle', text: '⚠️' });
    });
    candleSeries.setMarkers([...allMarkers, ...userMarkers].sort((a, b) => a.time - b.time));
}

// --- Math Functions ---
function calculateEMA(data, p) {
    const k = 2 / (p + 1);
    let ema = data[0].close;
    return data.map((c, i) => {
        ema = (i === 0) ? c.close : (c.close * k) + (ema * (1 - k));
        return { time: c.time, value: ema };
    });
}

function calculateWMA(data, period) {
    const isObj = data[0] && typeof data[0] === 'object';
    const vals = isObj ? data.map(d => d.close) : data;
    const times = isObj ? data.map(d => d.time) : null;
    const res = [];

    for (let i = 0; i < vals.length; i++) {
        if (i < period - 1) {
            res.push(0);
            continue;
        }
        let num = 0, den = 0;
        for (let j = 0; j < period; j++) {
            const w = period - j;
            num += vals[i - j] * w;
            den += w;
        }
        res.push(num / den);
    }

    // ถ้าเป็น object ให้ return พร้อม time
    if (times) {
        return res.map((value, i) => ({ time: times[i], value: value }));
    }
    return res;
}

function calculateHMA(data, period) {
    const half = Math.max(1, Math.floor(period / 2));
    const sqrt = Math.max(1, Math.floor(Math.sqrt(period)));

    const wmaHalf = calculateWMA(data, half);
    const wmaFull = calculateWMA(data, period);

    // คำนวณ raw โดยใช้ค่าจาก wmaHalf และ wmaFull
    const raw = data.map((d, i) => {
        const halfVal = wmaHalf[i].value;
        const fullVal = wmaFull[i].value;
        if (halfVal === 0 || fullVal === 0) {
            return { time: d.time, close: 0 };
        }
        return { time: d.time, close: 2 * halfVal - fullVal };
    });

    // คำนวณ WMA อีกครั้งจาก raw
    const result = calculateWMA(raw, sqrt);

    return result;
}

function calculateEHMA(data, period) {
    const half = Math.max(1, Math.floor(period / 2));
    const sqrt = Math.max(1, Math.floor(Math.sqrt(period)));

    const emaHalf = calculateEMA(data, half);
    const emaFull = calculateEMA(data, period);

    const raw = data.map((d, i) => ({
        time: d.time,
        close: 2 * emaHalf[i].value - emaFull[i].value
    }));

    return calculateEMA(raw, sqrt);
}


function calculateBB(data, p) {
    let u = [], m = [], l = []; if (data.length < p) return { upper: [], middle: [], lower: [] };
    for (let i = p - 1; i < data.length; i++) {
        const slice = data.slice(i - p + 1, i + 1).map(c => c.close);
        const avg = slice.reduce((a, b) => a + b) / p;
        const std = Math.sqrt(slice.map(x => Math.pow(x - avg, 2)).reduce((a, b) => a + b) / p);
        u.push({ time: data[i].time, value: avg + (2 * std) }); m.push({ time: data[i].time, value: avg }); l.push({ time: data[i].time, value: avg - (2 * std) });
    }
    return { upper: u, middle: m, lower: l };
}

function calculateATR(data, p) {
    let atr = [], avg = 0;
    for (let i = 0; i < data.length; i++) {
        const tr = i === 0 ? data[i].high - data[i].low : Math.max(data[i].high - data[i].low, Math.abs(data[i].high - data[i - 1].close), Math.abs(data[i].low - data[i - 1].close));
        avg = i < p ? ((avg * i) + tr) / (i + 1) : ((avg * (p - 1)) + tr) / p;
        atr.push(avg);
    }
    return atr;
}

function calculateATRWithTime(data, p) {
    const atrValues = calculateATR(data, p);
    return data.map((c, i) => ({ time: c.time, value: atrValues[i] }));
}

function calculateCI(data, p) {
    if (data.length < p) return [];
    const atr = calculateATR(data, p);
    let res = [];
    for (let i = p - 1; i < data.length; i++) {
        const slice = data.slice(i - p + 1, i + 1);
        const high = Math.max(...slice.map(c => c.high)), low = Math.min(...slice.map(c => c.low));
        const sumATR = atr.slice(i - p + 1, i + 1).reduce((a, b) => a + b, 0);
        const ci = (high - low) > 0 ? 100 * (Math.log10(sumATR / (high - low)) / Math.log10(p)) : 0;
        res.push({ time: data[i].time, value: ci });
    }
    return res;
}

function calculateADX(data, p) {
    if (data.length < p * 2) return data.map(d => ({ time: d.time, value: 0 }));
    let adxRes = [];
    let trSum = 0, pdmSum = 0, mdmSum = 0;
    let dxValues = [];

    for (let i = 1; i < data.length; i++) {
        const upMove = data[i].high - data[i - 1].high;
        const downMove = data[i - 1].low - data[i].low;
        const pdm = (upMove > downMove && upMove > 0) ? upMove : 0;
        const mdm = (downMove > upMove && downMove > 0) ? downMove : 0;
        const tr = Math.max(data[i].high - data[i].low, Math.abs(data[i].high - data[i - 1].close), Math.abs(data[i].low - data[i - 1].close));

        if (i <= p) { trSum += tr; pdmSum += pdm; mdmSum += mdm; }
        else { trSum = trSum - (trSum / p) + tr; pdmSum = pdmSum - (pdmSum / p) + pdm; mdmSum = mdmSum - (mdmSum / p) + mdm; }

        if (i >= p) {
            const diPlus = (pdmSum / trSum) * 100;
            const diMinus = (mdmSum / trSum) * 100;
            const dx = Math.abs(diPlus - diMinus) / (diPlus + diMinus) * 100;
            dxValues.push({ time: data[i].time, value: dx });
        }
    }

    let adx = 0;
    for (let j = 0; j < dxValues.length; j++) {
        if (j < p) adx += dxValues[j].value / p;
        else adx = ((adx * (p - 1)) + dxValues[j].value) / p;
        if (j >= p) adxRes.push({ time: dxValues[j].time, value: adx });
    }
    return adxRes;
}

function generateAnalysisData() {
    if (!candleData || candleData.length === 0) {
        alert("กรุณาโหลดข้อมูล Candle ก่อน!");
        return [];
    }

    const atrMultiplier = parseFloat(document.getElementById('atrMultiplier')?.value) || 2;
    const bbPeriod = parseInt(document.getElementById('bbPeriod')?.value) || 20;

    // Get BB data
    const bbData = calculateBB(candleData, bbPeriod);

    // Build analysis array
    analysisArray = [];

    // Track the last EMA crossover index for calculating distance
    let lastEmaCutIndex = null;

    for (let i = 0; i < candleData.length; i++) {
        const candle = candleData[i];
        const prevCandle = i > 0 ? candleData[i - 1] : null;
        const nextCandle = i < candleData.length - 1 ? candleData[i + 1] : null;

        // 1. candletime
        const candletime = candle.time;

        // 2. candletimeDisplay - format to readable datetime
        const date = new Date((candle.time) * 1000);
        const candletimeDisplay = date.toLocaleString('th-TH', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit'
        });

        // 3. OHLC
        const open = candle.open;
        const high = candle.high;
        const low = candle.low;
        const close = candle.close;

        // 4. color
        let color = 'Equal';
        if (close > open) color = 'Green';
        else if (close < open) color = 'Red';

        // 5. nextColor
        let nextColor = null;
        if (nextCandle) {
            if (nextCandle.close > nextCandle.open) nextColor = 'Green';
            else if (nextCandle.close < nextCandle.open) nextColor = 'Red';
            else nextColor = 'Equal';
        }

        // 6. pipSize (full candle size)
        const pipSize = Math.abs(close - open);

        // 7. emaShortValue
        const emaShortValue = currentMaData[0] && currentMaData[0][i] ? currentMaData[0][i].value : null;

        // 8. emaShortDirection
        let emaShortDirection = 'Flat';
        if (i > 0 && currentMaData[0] && currentMaData[0][i] && currentMaData[0][i - 1]) {
            const diff = currentMaData[0][i].value - currentMaData[0][i - 1].value;
            if (diff > 0.0001) emaShortDirection = 'Up';
            else if (diff < -0.0001) emaShortDirection = 'Down';
        }

        // 9. emaShortTurnType
        let emaShortTurnType = '-';
        if (i >= 2 && currentMaData[0] && currentMaData[0][i] && currentMaData[0][i - 1] && currentMaData[0][i - 2]) {
            const currDiff = currentMaData[0][i].value - currentMaData[0][i - 1].value;
            const prevDiff = currentMaData[0][i - 1].value - currentMaData[0][i - 2].value;
            const currDir = currDiff > 0.0001 ? 'Up' : (currDiff < -0.0001 ? 'Down' : 'Flat');
            const prevDir = prevDiff > 0.0001 ? 'Up' : (prevDiff < -0.0001 ? 'Down' : 'Flat');

            if (currDir === 'Up' && prevDir === 'Down') emaShortTurnType = 'TurnUp';
            else if (currDir === 'Down' && prevDir === 'Up') emaShortTurnType = 'TurnDown';
        }

        // 10. emaMediumValue
        const emaMediumValue = currentMaData[1] && currentMaData[1][i] ? currentMaData[1][i].value : null;

        // 11. emaMediumDirection
        let emaMediumDirection = 'Flat';
        if (i > 0 && currentMaData[1] && currentMaData[1][i] && currentMaData[1][i - 1]) {
            const diff = currentMaData[1][i].value - currentMaData[1][i - 1].value;
            if (diff > 0.0001) emaMediumDirection = 'Up';
            else if (diff < -0.0001) emaMediumDirection = 'Down';
        }

        // 12. emaLongValue
        const emaLongValue = currentMaData[2] && currentMaData[2][i] ? currentMaData[2][i].value : null;

        // 13. emaLongDirection
        let emaLongDirection = 'Flat';
        if (i > 0 && currentMaData[2] && currentMaData[2][i] && currentMaData[2][i - 1]) {
            const diff = currentMaData[2][i].value - currentMaData[2][i - 1].value;
            if (diff > 0.0001) emaLongDirection = 'Up';
            else if (diff < -0.0001) emaLongDirection = 'Down';
        }

        // 14. emaAbove (Short above Medium?)
        let emaAbove = null;
        if (emaShortValue !== null && emaMediumValue !== null) {
            emaAbove = emaShortValue > emaMediumValue ? 'ShortAbove' : 'MediumAbove';
        }

        // 15. emaLongAbove (Medium above Long?)
        let emaLongAbove = null;
        if (emaMediumValue !== null && emaLongValue !== null) {
            emaLongAbove = emaMediumValue > emaLongValue ? 'MediumAbove' : 'LongAbove';
        }

        // 16. macd12 = abs(emaShortValue - emaMediumValue)
        let macd12Value = null;
        if (emaShortValue !== null && emaMediumValue !== null) {
            macd12Value = Math.abs(emaShortValue - emaMediumValue);
        }

        // 17. macd23 = abs(emaMediumValue - emaLongValue)
        let macd23Value = null;
        if (emaMediumValue !== null && emaLongValue !== null) {
            macd23Value = Math.abs(emaMediumValue - emaLongValue);
        }

        // NEW: previousEmaShortValue - ค่า EMA Short ของแท่งก่อนหน้า
        const previousEmaShortValue = (i > 0 && currentMaData[0] && currentMaData[0][i - 1])
            ? currentMaData[0][i - 1].value : null;

        // NEW: previousEmaMediumValue - ค่า EMA Medium ของแท่งก่อนหน้า
        const previousEmaMediumValue = (i > 0 && currentMaData[1] && currentMaData[1][i - 1])
            ? currentMaData[1][i - 1].value : null;

        // NEW: previousEmaLongValue - ค่า EMA Long ของแท่งก่อนหน้า
        const previousEmaLongValue = (i > 0 && currentMaData[2] && currentMaData[2][i - 1])
            ? currentMaData[2][i - 1].value : null;

        // NEW: previousMacd12 - ค่า MACD12 ของแท่งก่อนหน้า
        let previousMacd12 = null;
        if (previousEmaShortValue !== null && previousEmaMediumValue !== null) {
            previousMacd12 = Math.abs(previousEmaShortValue - previousEmaMediumValue);
        }

        // NEW: previousMacd23 - ค่า MACD23 ของแท่งก่อนหน้า
        let previousMacd23 = null;
        if (previousEmaMediumValue !== null && previousEmaLongValue !== null) {
            previousMacd23 = Math.abs(previousEmaMediumValue - previousEmaLongValue);
        }

        // NEW: emaConvergenceType - divergence เมื่อ macd12 ปัจจุบัน > previousMacd12, convergence เมื่อ < previousMacd12
        let emaConvergenceType = null;
        if (macd12Value !== null && previousMacd12 !== null) {
            if (macd12Value > previousMacd12) {
                emaConvergenceType = 'divergence';  // เส้น EMA กำลังแยกตัวออกจากกัน
            } else if (macd12Value < previousMacd12) {
                emaConvergenceType = 'convergence'; // เส้น EMA กำลังเข้าหากัน
            } else {
                emaConvergenceType = 'neutral';     // เท่ากัน
            }
        }

        // NEW: emaLongConvergenceType - เปรียบเทียบระหว่าง emaMedium กับ emaLong
        let emaLongConvergenceType = null;
        if (macd23Value !== null && previousMacd23 !== null) {
            if (macd23Value > previousMacd23) {
                emaLongConvergenceType = 'divergence';  // เส้น EMA กำลังแยกตัวออกจากกัน
            } else if (macd23Value < previousMacd23) {
                emaLongConvergenceType = 'convergence'; // เส้น EMA กำลังเข้าหากัน
            } else {
                emaLongConvergenceType = 'neutral';     // เท่ากัน
            }
        }

        // NEW FIELD 1: emaCutLongType - ตรวจจับจุดตัดระหว่าง emaLong กับ emaMedium
        let emaCutLongType = null;
        if (i > 0 && emaLongValue !== null && emaMediumValue !== null) {
            const prevEmaLong = currentMaData[2] && currentMaData[2][i - 1] ? currentMaData[2][i - 1].value : null;
            const prevEmaMedium = currentMaData[1] && currentMaData[1][i - 1] ? currentMaData[1][i - 1].value : null;

            if (prevEmaLong !== null && prevEmaMedium !== null) {
                // ตรวจสอบการตัดกัน - เส้นสลับตำแหน่งกัน
                const currentMediumAbove = emaMediumValue > emaLongValue;
                const prevMediumAbove = prevEmaMedium > prevEmaLong;

                if (currentMediumAbove !== prevMediumAbove) {
                    // มีการตัดกันเกิดขึ้น
                    if (currentMediumAbove) {
                        // emaMedium ตัดขึ้นเหนือ emaLong = Golden Cross = UpTrend
                        emaCutLongType = 'UpTrend';
                    } else {
                        // emaMedium ตัดลงใต้ emaLong = Death Cross = DownTrend
                        emaCutLongType = 'DownTrend';
                    }
                }
            }
        }

        // Update lastEmaCutIndex when crossover detected
        if (emaCutLongType !== null) {
            lastEmaCutIndex = i;
        }

        // NEW FIELD 2: candlesSinceEmaCut - จำนวนแท่งห่างจากจุดตัดล่าสุด
        let candlesSinceEmaCut = null;
        if (lastEmaCutIndex !== null) {
            candlesSinceEmaCut = i - lastEmaCutIndex;
        }

        // 14. choppyIndicator (CI)
        const ciData = currentCiValues.find(v => v.time === candle.time);
        const choppyIndicator = ciData ? ciData.value : null;

        // 15. adxValue
        const adxData = currentAdxValues.find(v => v.time === candle.time);
        const adxValue = adxData ? adxData.value : null;

        // 16. BB (Bollinger Bands values)
        let bbValues = { upper: null, middle: null, lower: null };
        const bbIdx = bbData.upper.findIndex(v => v.time === candle.time);
        if (bbIdx !== -1) {
            bbValues = {
                upper: bbData.upper[bbIdx].value,
                middle: bbData.middle[bbIdx].value,
                lower: bbData.lower[bbIdx].value
            };
        }

        // 17. ตำแหน่งราคาเทียบกับ BB
        let bbPosition = 'Unknown';
        if (bbValues.upper !== null && bbValues.lower !== null) {
            const bbRange = bbValues.upper - bbValues.lower;
            const upperZone = bbValues.upper - (bbRange * 0.33);
            const lowerZone = bbValues.lower + (bbRange * 0.33);

            if (close >= upperZone) bbPosition = 'NearUpper';
            else if (close <= lowerZone) bbPosition = 'NearLower';
            else bbPosition = 'Middle';
        }

        // 18. atr
        const atrData = currentAtrValues.find(v => v.time === candle.time);
        const atr = atrData ? atrData.value : null;

        // 19. isAbnormalCandle - ใช้ True Range เทียบกับ ATR x multiplier
        let isAbnormalCandle = false;
        if (atr !== null && prevCandle) {
            const trueRange = Math.max(
                high - low,
                Math.abs(high - prevCandle.close),
                Math.abs(low - prevCandle.close)
            );
            isAbnormalCandle = trueRange > (atr * atrMultiplier);
        }

        // 20. UWick (Upper Wick)
        const bodyTop = Math.max(open, close);
        const bodyBottom = Math.min(open, close);
        const uWick = high - bodyTop;

        // 21. Body
        const body = Math.abs(close - open);

        // 23. LWick (Lower Wick) - note: 22 skipped in requirements
        const lWick = bodyBottom - low;

        // 24. ตำแหน่งที่ emaShortValue ตัดผ่าน
        let emaCutPosition = null;
        if (emaShortValue !== null) {
            // 1. ถ้าตัดผ่านเหนือ UWick ให้เป็น 1
            if (emaShortValue > high) {
                emaCutPosition = '1'; // Above Upper Wick
            }
            // 2. ถ้าตัดผ่าน UWick กับ Close/Open ให้เป็น 2
            else if (emaShortValue >= bodyTop && emaShortValue <= high) {
                emaCutPosition = '2'; // Between Upper Wick and Body Top
            }
            // 3. ถ้าตัดผ่าน Body - แบ่งเป็น 3 ส่วน (B1, B2, B3)
            else if (emaShortValue >= bodyBottom && emaShortValue < bodyTop) {
                const bodyRange = bodyTop - bodyBottom;
                if (bodyRange > 0) {
                    const positionInBody = (emaShortValue - bodyBottom) / bodyRange;
                    if (positionInBody >= 0.66) {
                        emaCutPosition = 'B1'; // Top 30% of body
                    } else if (positionInBody >= 0.33) {
                        emaCutPosition = 'B2'; // Middle 30% of body
                    } else {
                        emaCutPosition = 'B3'; // Bottom 30% of body
                    }
                } else {
                    emaCutPosition = 'B2'; // Doji - equal open/close
                }
            }
            // 4. ถ้าตัดผ่าน LWick กับ Open/Close ให้เป็น 3
            else if (emaShortValue >= low && emaShortValue < bodyBottom) {
                emaCutPosition = '3'; // Between Body Bottom and Lower Wick
            }
            // 5. ถ้าตัดผ่านใต้ LWick ให้เป็น 4
            else if (emaShortValue < low) {
                emaCutPosition = '4'; // Below Lower Wick
            }
        }

        // Candle body percentages
        const fullCandleSize = high - low;
        const bodyPercent = fullCandleSize > 0 ? ((body / fullCandleSize) * 100).toFixed(2) : 0;
        const uWickPercent = fullCandleSize > 0 ? ((uWick / fullCandleSize) * 100).toFixed(2) : 0;
        const lWickPercent = fullCandleSize > 0 ? ((lWick / fullCandleSize) * 100).toFixed(2) : 0;

        // Build analysis object
        const analysisObj = {
            index: i,
            candletime: candletime,
            candletimeDisplay: candletimeDisplay,
            open: open,
            high: high,
            low: low,
            close: close,
            color: color,
            nextColor: nextColor,
            pipSize: parseFloat(pipSize.toFixed(5)),
            emaShortValue: emaShortValue !== null ? parseFloat(emaShortValue.toFixed(5)) : null,
            emaShortDirection: emaShortDirection,
            emaShortTurnType: emaShortTurnType,
            emaMediumValue: emaMediumValue !== null ? parseFloat(emaMediumValue.toFixed(5)) : null,
            emaMediumDirection: emaMediumDirection,
            emaLongValue: emaLongValue !== null ? parseFloat(emaLongValue.toFixed(5)) : null,
            emaLongDirection: emaLongDirection,
            emaAbove: emaAbove,
            emaLongAbove: emaLongAbove,
            macd12: macd12Value !== null ? parseFloat(macd12Value.toFixed(5)) : null,
            macd23: macd23Value !== null ? parseFloat(macd23Value.toFixed(5)) : null,
            previousEmaShortValue: previousEmaShortValue !== null ? parseFloat(previousEmaShortValue.toFixed(5)) : null,
            previousEmaMediumValue: previousEmaMediumValue !== null ? parseFloat(previousEmaMediumValue.toFixed(5)) : null,
            previousEmaLongValue: previousEmaLongValue !== null ? parseFloat(previousEmaLongValue.toFixed(5)) : null,
            previousMacd12: previousMacd12 !== null ? parseFloat(previousMacd12.toFixed(5)) : null,
            previousMacd23: previousMacd23 !== null ? parseFloat(previousMacd23.toFixed(5)) : null,
            emaConvergenceType: emaConvergenceType,
            emaLongConvergenceType: emaLongConvergenceType,
            choppyIndicator: choppyIndicator !== null ? parseFloat(choppyIndicator.toFixed(2)) : null,
            adxValue: adxValue !== null ? parseFloat(adxValue.toFixed(2)) : null,
            bbValues: {
                upper: bbValues.upper !== null ? parseFloat(bbValues.upper.toFixed(5)) : null,
                middle: bbValues.middle !== null ? parseFloat(bbValues.middle.toFixed(5)) : null,
                lower: bbValues.lower !== null ? parseFloat(bbValues.lower.toFixed(5)) : null
            },
            bbPosition: bbPosition,
            atr: atr !== null ? parseFloat(atr.toFixed(5)) : null,
            isAbnormalCandle: isAbnormalCandle,
            uWick: parseFloat(uWick.toFixed(5)),
            uWickPercent: parseFloat(uWickPercent),
            body: parseFloat(body.toFixed(5)),
            bodyPercent: parseFloat(bodyPercent),
            lWick: parseFloat(lWick.toFixed(5)),
            lWickPercent: parseFloat(lWickPercent),
            emaCutPosition: emaCutPosition,
            emaCutLongType: emaCutLongType,
            candlesSinceEmaCut: candlesSinceEmaCut
        };

        analysisArray.push(analysisObj);
    }

    // Output to NEW textarea (not overwriting the old one)
    console.log('📊 Analysis Data Generated:', analysisArray);
    document.getElementById("analysisDataTxt").value = JSON.stringify(analysisArray, null, 2);

    // Calculate summary statistics
    const abnormalCount = analysisArray.filter(a => a.isAbnormalCandle).length;
    const greenCount = analysisArray.filter(a => a.color === 'Green').length;
    const redCount = analysisArray.filter(a => a.color === 'Red').length;
    const emaCrossoverCount = analysisArray.filter(a => a.emaCutLongType !== null).length;
    const upTrendCount = analysisArray.filter(a => a.emaCutLongType === 'UpTrend').length;
    const downTrendCount = analysisArray.filter(a => a.emaCutLongType === 'DownTrend').length;

    // Update summary UI
    document.getElementById('analysisCount').textContent = analysisArray.length;
    document.getElementById('emaCrossCount').textContent = emaCrossoverCount;
    document.getElementById('upTrendCount').textContent = upTrendCount;
    document.getElementById('downTrendCount').textContent = downTrendCount;

    // Update status cards with latest values
    if (analysisArray.length > 0) {
        const latest = analysisArray[analysisArray.length - 1];

        // Update CI card
        if (latest.choppyIndicator !== null) {
            document.getElementById('ciValue').textContent = latest.choppyIndicator.toFixed(1);
        }

        // Update ADX card
        if (latest.adxValue !== null) {
            document.getElementById('adxValue').textContent = parseFloat(latest.adxValue).toFixed(1);
        }

        // Update BB Width card
        if (latest.bbValues.upper !== null && latest.bbValues.lower !== null) {
            const bbWidth = (parseFloat(latest.bbValues.upper) - parseFloat(latest.bbValues.lower)).toFixed(4);
            document.getElementById('bbValue').textContent = bbWidth;
        }

        // Update Market State
        let marketState = 'WAIT';
        if (latest.choppyIndicator !== null && latest.adxValue !== null) {
            const ci = latest.choppyIndicator;
            const adx = parseFloat(latest.adxValue);
            if (ci < 38.2 && adx > 25) {
                marketState = 'TREND';
            } else if (ci > 61.8 || adx < 20) {
                marketState = 'RANGE';
            } else {
                marketState = 'NEUTRAL';
            }
        }
        document.getElementById('marketState').textContent = marketState;

        // Update Marker count
        document.getElementById('markerCount').textContent = emaCrossoverCount;
    }

    alert(`📊 Analysis Data Generated!\n\n` +
        `Total Candles: ${analysisArray.length}\n` +
        `Green: ${greenCount}\n` +
        `Red: ${redCount}\n` +
        `Abnormal (ATR x${atrMultiplier}): ${abnormalCount}\n` +
        `EMA Crossovers: ${emaCrossoverCount} (⬆${upTrendCount} / ⬇${downTrendCount})\n\n` +
        `Data saved to Analysis Data textarea below.`);

    //resultAlter = mainAlterColorAnaly(candleData) ;
    //document.getElementById("historyList").innerHTML = JSON.stringify(resultAlter);;

    return analysisArray;
}