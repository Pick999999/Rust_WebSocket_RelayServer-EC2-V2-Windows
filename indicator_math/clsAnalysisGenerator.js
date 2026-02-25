/**
 * AnalysisGenerator Class
 * สร้างข้อมูลวิเคราะห์จาก candleData array
 * สามารถนำไปใช้ในโปรเจคอื่นๆ ได้โดยไม่ต้องพึ่งพา DOM
 *
 * ================== AVAILABLE FUNCTIONS ==================
 *
 * Core Methods:
 * - constructor(candleData, options)  : กำหนดค่าเริ่มต้นและ options ต่างๆ
 * - generate()                        : สร้างข้อมูลวิเคราะห์ (Analysis Data) ทั้งหมด
 * - getSummary()                      : ดึงข้อมูลสรุปสถิติ (Count, Trends, Latest Values)
 * - toJSON()                          : แปลงข้อมูล Analysis Data เป็น JSON String
 *
 * Indicator Calculations:
 * - calculateMA(data, period, type)   : คำนวณ Moving Average ตามประเภทที่ระบุ (EMA, HMA, EHMA)
 * - calculateEMA(data, period)        : คำนวณ Exponential Moving Average
 * - calculateHMA(data, period)        : คำนวณ Hull Moving Average
 * - calculateEHMA(data, period)       : คำนวณ Exponential Hull Moving Average
 * - calculateWMA(data, period)        : คำนวณ Weighted Moving Average (Helper for HMA)
 * - calculateRSI(data, period)        : คำนวณ Relative Strength Index
 * - calculateATR(data, period)        : คำนวณ Average True Range
 * - calculateATRWithTime(data, period): คำนวณ ATR โดยคงโครงสร้าง Time ไว้
 * - calculateBB(data, period)         : คำนวณ Bollinger Bands (Upper, Middle, Lower)
 * - calculateCI(data, period)         : คำนวณ Choppiness Index
 * - calculateADX(data, period)        : คำนวณ Average Directional Index
 *
 * Helper Methods:
 * - getEMADirection(prev, curr)       : หาแนวโน้มของ EMA (Up, Down, Flat)
 * - getMACDConver(prevMACD, currMACD) : หาการลู่เข้า/ออกของ MACD (Convergence/Divergence)
 *
 * ========================================================
 *
 * Usage:
 *   const generator = new AnalysisGenerator(candleData, {
 *     ema1Period: 20,
 *     ema1Type: 'EMA',    // 'EMA', 'HMA', 'EHMA'
 *     ema2Period: 50,
 *     ema2Type: 'EMA',    // 'EMA', 'HMA', 'EHMA'
 *     ema3Period: 200,
 *     ema3Type: 'EMA',    // 'EMA', 'HMA', 'EHMA'
 *     atrPeriod: 14,
 *     atrMultiplier: 2,
 *     bbPeriod: 20,
 *     ciPeriod: 14,
 *     adxPeriod: 14,
 *     rsiPeriod: 14,
 *     flatThreshold: 0.2,
 *     macdNarrow: 0.15
 *   });
 *   const analysisData = generator.generate();
 */

class AnalysisGenerator {
    constructor(candleData, options = {}) {
        this.candleData = candleData || [];

        // Default options
        this.options = {
            ema1Period: options.ema1Period || 20,
            ema1Type: (options.ema1Type || 'EMA').toUpperCase(),  // 'EMA', 'HMA', 'EHMA'
            ema2Period: options.ema2Period || 50,
            ema2Type: (options.ema2Type || 'EMA').toUpperCase(),  // 'EMA', 'HMA', 'EHMA'
            ema3Period: options.ema3Period || 200,
            ema3Type: (options.ema3Type || 'EMA').toUpperCase(),  // 'EMA', 'HMA', 'EHMA'
            atrPeriod: options.atrPeriod || 14,
            atrMultiplier: options.atrMultiplier || 2,
            bbPeriod: options.bbPeriod || 20,
            ciPeriod: options.ciPeriod || 14,
            adxPeriod: options.adxPeriod || 14,
            rsiPeriod: options.rsiPeriod || 14,
            flatThreshold: options.flatThreshold || 0.2,
            macdNarrow: options.macdNarrow || 0.15
        };

        // Calculated indicator data
        this.ema1Data = [];
        this.ema2Data = [];
        this.ema3Data = [];
        this.atrData = [];
        this.ciData = [];
        this.adxData = [];
        this.rsiData = [];
        this.bbData = { upper: [], middle: [], lower: [] };

        // Analysis result
        this.analysisArray = [];
    }

    // ================== INDICATOR CALCULATION METHODS ==================

    /**
     * คำนวณ EMA (Exponential Moving Average)
     */
    calculateEMA(data, period) {
        const k = 2 / (period + 1);
        let ema = data[0].close;
        return data.map((c, i) => {
            ema = (i === 0) ? c.close : (c.close * k) + (ema * (1 - k));
            return { time: c.time, value: ema };
        });
    }

    /**
     * คำนวณ WMA (Weighted Moving Average)
     */
    calculateWMA(data, period) {
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

        if (times) {
            return res.map((value, i) => ({ time: times[i], value: value }));
        }
        return res;
    }

    /**
     * คำนวณ HMA (Hull Moving Average)
     */
    calculateHMA(data, period) {
        const half = Math.max(1, Math.floor(period / 2));
        const sqrt = Math.max(1, Math.floor(Math.sqrt(period)));

        const wmaHalf = this.calculateWMA(data, half);
        const wmaFull = this.calculateWMA(data, period);

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
        return this.calculateWMA(raw, sqrt);
    }

    /**
     * คำนวณ EHMA (Exponential Hull Moving Average)
     */
    calculateEHMA(data, period) {
        const half = Math.max(1, Math.floor(period / 2));
        const sqrt = Math.max(1, Math.floor(Math.sqrt(period)));

        const emaHalf = this.calculateEMA(data, half);
        const emaFull = this.calculateEMA(data, period);

        const raw = data.map((d, i) => ({
            time: d.time,
            close: 2 * emaHalf[i].value - emaFull[i].value
        }));

        return this.calculateEMA(raw, sqrt);
    }

    /**
     * คำนวณ RSI (Relative Strength Index)
     */
    calculateRSI(data, period) {
        if (data.length < period + 1) return [];

        const result = [];
        let gains = [];
        let losses = [];

        // คำนวณ gain และ loss สำหรับแต่ละแท่ง
        for (let i = 1; i < data.length; i++) {
            const change = data[i].close - data[i - 1].close;
            gains.push(change > 0 ? change : 0);
            losses.push(change < 0 ? Math.abs(change) : 0);
        }

        // คำนวณ average gain และ average loss แรก
        let avgGain = gains.slice(0, period).reduce((a, b) => a + b, 0) / period;
        let avgLoss = losses.slice(0, period).reduce((a, b) => a + b, 0) / period;

        // RSI สำหรับ period แรก
        let rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
        let rsi = 100 - (100 / (1 + rs));
        result.push({ time: data[period].time, value: rsi });

        // คำนวณ RSI ต่อไปโดยใช้ smoothed averages
        for (let i = period; i < gains.length; i++) {
            avgGain = ((avgGain * (period - 1)) + gains[i]) / period;
            avgLoss = ((avgLoss * (period - 1)) + losses[i]) / period;

            rs = avgLoss === 0 ? 100 : avgGain / avgLoss;
            rsi = 100 - (100 / (1 + rs));
            result.push({ time: data[i + 1].time, value: rsi });
        }

        return result;
    }

    /**
     * เลือก MA type และคำนวณ
     */
    calculateMA(data, period, type) {
        switch (type.toUpperCase()) {
            case 'HMA':
                return this.calculateHMA(data, period);
            case 'EHMA':
                return this.calculateEHMA(data, period);
            case 'EMA':
            default:
                return this.calculateEMA(data, period);
        }
    }

    /**
     * คำนวณ ATR (Average True Range)
     */
    calculateATR(data, period) {
        let atr = [], avg = 0;
        for (let i = 0; i < data.length; i++) {
            const tr = i === 0
                ? data[i].high - data[i].low
                : Math.max(
                    data[i].high - data[i].low,
                    Math.abs(data[i].high - data[i - 1].close),
                    Math.abs(data[i].low - data[i - 1].close)
                );
            avg = i < period ? ((avg * i) + tr) / (i + 1) : ((avg * (period - 1)) + tr) / period;
            atr.push(avg);
        }
        return atr;
    }

    /**
     * คำนวณ ATR พร้อม time
     */
    calculateATRWithTime(data, period) {
        const atrValues = this.calculateATR(data, period);
        return data.map((c, i) => ({ time: c.time, value: atrValues[i] }));
    }

    /**
     * คำนวณ Bollinger Bands
     */
    calculateBB(data, period) {
        let upper = [], middle = [], lower = [];
        if (data.length < period) return { upper: [], middle: [], lower: [] };

        for (let i = period - 1; i < data.length; i++) {
            const slice = data.slice(i - period + 1, i + 1).map(c => c.close);
            const avg = slice.reduce((a, b) => a + b) / period;
            const std = Math.sqrt(slice.map(x => Math.pow(x - avg, 2)).reduce((a, b) => a + b) / period);
            upper.push({ time: data[i].time, value: avg + (2 * std) });
            middle.push({ time: data[i].time, value: avg });
            lower.push({ time: data[i].time, value: avg - (2 * std) });
        }
        return { upper, middle, lower };
    }

    /**
     * คำนวณ Choppiness Index
     */
    calculateCI(data, period) {
        if (data.length < period) return [];
        const atr = this.calculateATR(data, period);
        let res = [];
        for (let i = period - 1; i < data.length; i++) {
            const slice = data.slice(i - period + 1, i + 1);
            const high = Math.max(...slice.map(c => c.high));
            const low = Math.min(...slice.map(c => c.low));
            const sumATR = atr.slice(i - period + 1, i + 1).reduce((a, b) => a + b, 0);
            const ci = (high - low) > 0 ? 100 * (Math.log10(sumATR / (high - low)) / Math.log10(period)) : 0;
            res.push({ time: data[i].time, value: ci });
        }
        return res;
    }

    /**
     * คำนวณ ADX (Average Directional Index)
     */
    calculateADX(data, period) {
        if (data.length < period * 2) return data.map(d => ({ time: d.time, value: 0 }));
        let adxRes = [];
        let trSum = 0, pdmSum = 0, mdmSum = 0;
        let dxValues = [];

        for (let i = 1; i < data.length; i++) {
            const upMove = data[i].high - data[i - 1].high;
            const downMove = data[i - 1].low - data[i].low;
            const pdm = (upMove > downMove && upMove > 0) ? upMove : 0;
            const mdm = (downMove > upMove && downMove > 0) ? downMove : 0;
            const tr = Math.max(
                data[i].high - data[i].low,
                Math.abs(data[i].high - data[i - 1].close),
                Math.abs(data[i].low - data[i - 1].close)
            );

            if (i <= period) {
                trSum += tr; pdmSum += pdm; mdmSum += mdm;
            } else {
                trSum = trSum - (trSum / period) + tr;
                pdmSum = pdmSum - (pdmSum / period) + pdm;
                mdmSum = mdmSum - (mdmSum / period) + mdm;
            }

            if (i >= period) {
                const diPlus = (pdmSum / trSum) * 100;
                const diMinus = (mdmSum / trSum) * 100;
                const dx = Math.abs(diPlus - diMinus) / (diPlus + diMinus) * 100;
                dxValues.push({ time: data[i].time, value: dx });
            }
        }

        let adx = 0;
        for (let j = 0; j < dxValues.length; j++) {
            if (j < period) adx += dxValues[j].value / period;
            else adx = ((adx * (period - 1)) + dxValues[j].value) / period;
            if (j >= period) adxRes.push({ time: dxValues[j].time, value: adx });
        }
        return adxRes;
    }

    // ================== HELPER METHODS ==================

    /**
     * หา EMA Direction จากค่าก่อนหน้าและปัจจุบัน
     */
    getEMADirection(previousEMA, currentEMA) {
        const diff = previousEMA - currentEMA;
        if (Math.abs(diff) <= this.options.flatThreshold) {
            return 'Flat';
        } else if (previousEMA < currentEMA) {
            return 'Up';
        } else {
            return 'Down';
        }
    }

    /**
     * หา MACD Convergence Type
     */
    getMACDConver(previousMACD, currentMACD) {
        currentMACD = parseFloat(currentMACD);
        previousMACD = parseFloat(previousMACD);

        if (currentMACD !== null && previousMACD !== null) {
            if (currentMACD <= this.options.macdNarrow) {
                return 'N'; // Narrow - แคบมาก
            }
            if (currentMACD > previousMACD) {
                return 'D'; // Divergence - แยกตัว
            }
            if (currentMACD < previousMACD) {
                return 'C'; // Convergence - เข้าหากัน
            }
        }
        return null;
    }

    // ================== MAIN GENERATION METHOD ==================

    /**
     * สร้าง Analysis Data จาก candleData
     * @returns {Array} analysisArray
     */
    generate() {
        if (!this.candleData || this.candleData.length === 0) {
            console.warn("AnalysisGenerator: No candle data provided!");
            return [];
        }

        // Calculate all indicators using selected MA types
        this.ema1Data = this.calculateMA(this.candleData, this.options.ema1Period, this.options.ema1Type);
        this.ema2Data = this.calculateMA(this.candleData, this.options.ema2Period, this.options.ema2Type);
        this.ema3Data = this.calculateMA(this.candleData, this.options.ema3Period, this.options.ema3Type);
        this.atrData = this.calculateATRWithTime(this.candleData, this.options.atrPeriod);
        this.ciData = this.calculateCI(this.candleData, this.options.ciPeriod);
        this.adxData = this.calculateADX(this.candleData, this.options.adxPeriod);
        this.rsiData = this.calculateRSI(this.candleData, this.options.rsiPeriod);
        this.bbData = this.calculateBB(this.candleData, this.options.bbPeriod);

        // Build analysis array
        this.analysisArray = [];
        let lastEmaCutIndex = null;

        // Track consecutive Up/Down counts for Medium and Long EMA
        let upConMediumEMA = 0;
        let downConMediumEMA = 0;
        let upConLongEMA = 0;
        let downConLongEMA = 0;

        for (let i = 0; i < this.candleData.length; i++) {
            const candle = this.candleData[i];
            const prevCandle = i > 0 ? this.candleData[i - 1] : null;
            const nextCandle = i < this.candleData.length - 1 ? this.candleData[i + 1] : null;

            // 1. candletime
            const candletime = candle.time;

            // 2. candletimeDisplay
            const date = new Date(candle.time * 1000);
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

            // 6. pipSize
            const pipSize = Math.abs(close - open);

            // 7-9. EMA Short
            const emaShortValue = this.ema1Data[i] ? this.ema1Data[i].value : null;
            let emaShortDirection = 'Flat';
            if (i > 0 && this.ema1Data[i] && this.ema1Data[i - 1]) {
                emaShortDirection = this.getEMADirection(this.ema1Data[i - 1].value, this.ema1Data[i].value);
            }

            let emaShortTurnType = '-';
            if (i >= 2 && this.ema1Data[i] && this.ema1Data[i - 1] && this.ema1Data[i - 2]) {
                const currDiff = this.ema1Data[i].value - this.ema1Data[i - 1].value;
                const prevDiff = this.ema1Data[i - 1].value - this.ema1Data[i - 2].value;
                const currDir = currDiff > 0.0001 ? 'Up' : (currDiff < -0.0001 ? 'Down' : 'Flat');
                const prevDir = prevDiff > 0.0001 ? 'Up' : (prevDiff < -0.0001 ? 'Down' : 'Flat');

                if (currDir === 'Up' && prevDir === 'Down') emaShortTurnType = 'TurnUp';
                else if (currDir === 'Down' && prevDir === 'Up') emaShortTurnType = 'TurnDown';
            }

            // 10-11. EMA Medium
            const emaMediumValue = this.ema2Data[i] ? this.ema2Data[i].value : null;
            let emaMediumDirection = 'Flat';
            if (i > 0 && this.ema2Data[i] && this.ema2Data[i - 1]) {
                emaMediumDirection = this.getEMADirection(this.ema2Data[i - 1].value, this.ema2Data[i].value);
            }

            // Track consecutive Up/Down for Medium EMA
            // UpCon = นับจำนวนแท่งที่ Direction เป็น Up ต่อเนื่องนับจาก Turn Up
            // DownCon = นับจำนวนแท่งที่ Direction เป็น Down ต่อเนื่องนับจาก Turn Down
            // Flat จะไม่ reset counter แต่จะเก็บค่าต่อไป
            if (emaMediumDirection === 'Up') {
                upConMediumEMA++;
                downConMediumEMA = 0;  // Reset DownCon เมื่อเปลี่ยนเป็น Up
            } else if (emaMediumDirection === 'Down') {
                downConMediumEMA++;
                upConMediumEMA = 0;    // Reset UpCon เมื่อเปลี่ยนเป็น Down
            }
            // else Flat - ไม่ทำอะไร (เก็บค่า counter ไว้เหมือนเดิม)

            // 12-13. EMA Long
            const emaLongValue = this.ema3Data[i] ? this.ema3Data[i].value : null;
            let emaLongDirection = 'Flat';
            if (i > 0 && this.ema3Data[i] && this.ema3Data[i - 1]) {
                emaLongDirection = this.getEMADirection(this.ema3Data[i - 1].value, this.ema3Data[i].value);
            }

            // Track consecutive Up/Down for Long EMA
            // เหมือนกันกับ Medium - Flat ไม่ reset counter
            if (emaLongDirection === 'Up') {
                upConLongEMA++;
                downConLongEMA = 0;  // Reset DownCon เมื่อเปลี่ยนเป็น Up
            } else if (emaLongDirection === 'Down') {
                downConLongEMA++;
                upConLongEMA = 0;    // Reset UpCon เมื่อเปลี่ยนเป็น Down
            }
            // else Flat - ไม่ทำอะไร (เก็บค่า counter ไว้เหมือนเดิม)

            // 14. emaAbove
            let emaAbove = null;
            if (emaShortValue !== null && emaMediumValue !== null) {
                emaAbove = emaShortValue > emaMediumValue ? 'ShortAbove' : 'MediumAbove';
            }

            // 15. emaLongAbove
            let emaLongAbove = null;
            if (emaMediumValue !== null && emaLongValue !== null) {
                emaLongAbove = emaMediumValue > emaLongValue ? 'MediumAbove' : 'LongAbove';
            }

            // 16. macd12
            let macd12Value = null;
            if (emaShortValue !== null && emaMediumValue !== null) {
                macd12Value = Math.abs(emaShortValue - emaMediumValue);
            }

            // 17. macd23
            let macd23Value = null;
            if (emaMediumValue !== null && emaLongValue !== null) {
                macd23Value = Math.abs(emaMediumValue - emaLongValue);
            }

            // Previous EMA values
            const previousEmaShortValue = (i > 0 && this.ema1Data[i - 1]) ? this.ema1Data[i - 1].value : null;
            const previousEmaMediumValue = (i > 0 && this.ema2Data[i - 1]) ? this.ema2Data[i - 1].value : null;
            const previousEmaLongValue = (i > 0 && this.ema3Data[i - 1]) ? this.ema3Data[i - 1].value : null;

            // Previous MACD values
            let previousMacd12 = null;
            if (previousEmaShortValue !== null && previousEmaMediumValue !== null) {
                previousMacd12 = Math.abs(previousEmaShortValue - previousEmaMediumValue);
            }

            let previousMacd23 = null;
            if (previousEmaMediumValue !== null && previousEmaLongValue !== null) {
                previousMacd23 = Math.abs(previousEmaMediumValue - previousEmaLongValue);
            }

            // emaConvergenceType
            let emaConvergenceType = null;
            if (macd12Value !== null && previousMacd12 !== null) {
                if (macd12Value > previousMacd12) {
                    emaConvergenceType = 'divergence';
                } else if (macd12Value < previousMacd12) {
                    emaConvergenceType = 'convergence';
                } else {
                    emaConvergenceType = 'neutral';
                }
            }

            // emaLongConvergenceType
            let emaLongConvergenceType = this.getMACDConver(previousMacd23, macd23Value);
            if (Math.abs(macd23Value) < 0.15) {
                emaLongConvergenceType = 'N';
            }

            // emaCutLongType - จุดตัด EMA Long กับ Medium
            let emaCutLongType = null;
            if (i > 0 && emaLongValue !== null && emaMediumValue !== null) {
                const prevEmaLong = this.ema3Data[i - 1] ? this.ema3Data[i - 1].value : null;
                const prevEmaMedium = this.ema2Data[i - 1] ? this.ema2Data[i - 1].value : null;

                if (prevEmaLong !== null && prevEmaMedium !== null) {
                    const currentMediumAbove = emaMediumValue > emaLongValue;
                    const prevMediumAbove = prevEmaMedium > prevEmaLong;

                    if (currentMediumAbove !== prevMediumAbove) {
                        if (currentMediumAbove) {
                            emaCutLongType = 'UpTrend';
                        } else {
                            emaCutLongType = 'DownTrend';
                        }
                    }
                }
            }

            if (emaCutLongType !== null) {
                lastEmaCutIndex = i;
            }

            // candlesSinceEmaCut
            let candlesSinceEmaCut = null;
            if (lastEmaCutIndex !== null) {
                candlesSinceEmaCut = i - lastEmaCutIndex;
            }

            // CI
            const ciData = this.ciData.find(v => v.time === candle.time);
            const choppyIndicator = ciData ? ciData.value : null;

            // ADX
            const adxData = this.adxData.find(v => v.time === candle.time);
            const adxValue = adxData ? adxData.value : null;

            // RSI
            const rsiData = this.rsiData.find(v => v.time === candle.time);
            const rsiValue = rsiData ? rsiData.value : null;

            // BB
            let bbValues = { upper: null, middle: null, lower: null };
            const bbIdx = this.bbData.upper.findIndex(v => v.time === candle.time);
            if (bbIdx !== -1) {
                bbValues = {
                    upper: this.bbData.upper[bbIdx].value,
                    middle: this.bbData.middle[bbIdx].value,
                    lower: this.bbData.lower[bbIdx].value
                };
            }

            // BB Position
            let bbPosition = 'Unknown';
            if (bbValues.upper !== null && bbValues.lower !== null) {
                const bbRange = bbValues.upper - bbValues.lower;
                const upperZone = bbValues.upper - (bbRange * 0.33);
                const lowerZone = bbValues.lower + (bbRange * 0.33);

                if (close >= upperZone) bbPosition = 'NearUpper';
                else if (close <= lowerZone) bbPosition = 'NearLower';
                else bbPosition = 'Middle';
            }

            // ATR
            const atrData = this.atrData.find(v => v.time === candle.time);
            const atr = atrData ? atrData.value : null;

            // isAbnormalCandle - True Range > ATR * multiplier
            let isAbnormalCandle = false;
            if (atr !== null && prevCandle) {
                const trueRange = Math.max(
                    high - low,
                    Math.abs(high - prevCandle.close),
                    Math.abs(low - prevCandle.close)
                );
                isAbnormalCandle = trueRange > (atr * this.options.atrMultiplier);
            }

            // NEW: isAbnormalATR - ตรวจสอบว่าแท่งเทียนมีขนาดผิดปกติโดยใช้ ATR
            // ใช้ขนาด body เทียบกับ ATR ด้วย (ไม่ใช่แค่ True Range)
            let isAbnormalATR = false;
            if (atr !== null && atr > 0) {
                const bodySize = Math.abs(close - open);
                const fullCandleSize = high - low;
                // ถือว่าผิดปกติถ้า body หรือ candle size มากกว่า ATR * multiplier
                isAbnormalATR = (bodySize > atr * this.options.atrMultiplier) ||
                    (fullCandleSize > atr * this.options.atrMultiplier * 1.5);
            }

            // Wick and Body calculations
            const bodyTop = Math.max(open, close);
            const bodyBottom = Math.min(open, close);
            const uWick = high - bodyTop;
            const body = Math.abs(close - open);
            const lWick = bodyBottom - low;

            // emaCutPosition
            let emaCutPosition = null;
            if (emaShortValue !== null) {
                if (emaShortValue > high) {
                    emaCutPosition = '1';
                } else if (emaShortValue >= bodyTop && emaShortValue <= high) {
                    emaCutPosition = '2';
                } else if (emaShortValue >= bodyBottom && emaShortValue < bodyTop) {
                    const bodyRange = bodyTop - bodyBottom;
                    if (bodyRange > 0) {
                        const positionInBody = (emaShortValue - bodyBottom) / bodyRange;
                        if (positionInBody >= 0.66) emaCutPosition = 'B1';
                        else if (positionInBody >= 0.33) emaCutPosition = 'B2';
                        else emaCutPosition = 'B3';
                    } else {
                        emaCutPosition = 'B2';
                    }
                } else if (emaShortValue >= low && emaShortValue < bodyBottom) {
                    emaCutPosition = '3';
                } else if (emaShortValue < low) {
                    emaCutPosition = '4';
                }
            }

            // Percentages
            const fullCandleSize = high - low;
            const bodyPercent = fullCandleSize > 0 ? ((body / fullCandleSize) * 100).toFixed(2) : 0;
            const uWickPercent = fullCandleSize > 0 ? ((uWick / fullCandleSize) * 100).toFixed(2) : 0;
            const lWickPercent = fullCandleSize > 0 ? ((lWick / fullCandleSize) * 100).toFixed(2) : 0;

            // StatusDesc
            const seriesDesc = (emaLongAbove ? emaLongAbove.substr(0, 1) : '-') + '-' +
                (emaMediumDirection ? emaMediumDirection.substr(0, 1) : '-') +
                (emaLongDirection ? emaLongDirection.substr(0, 1) : '-') + '-' +
                color.substr(0, 1) + '-' + (emaLongConvergenceType || '-');

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
                rsiValue: rsiValue !== null ? parseFloat(rsiValue.toFixed(2)) : null,
                bbValues: {
                    upper: bbValues.upper !== null ? parseFloat(bbValues.upper.toFixed(5)) : null,
                    middle: bbValues.middle !== null ? parseFloat(bbValues.middle.toFixed(5)) : null,
                    lower: bbValues.lower !== null ? parseFloat(bbValues.lower.toFixed(5)) : null
                },
                bbPosition: bbPosition,
                atr: atr !== null ? parseFloat(atr.toFixed(5)) : null,
                isAbnormalCandle: isAbnormalCandle,
                isAbnormalATR: isAbnormalATR,
                uWick: parseFloat(uWick.toFixed(5)),
                uWickPercent: parseFloat(uWickPercent),
                body: parseFloat(body.toFixed(5)),
                bodyPercent: parseFloat(bodyPercent),
                lWick: parseFloat(lWick.toFixed(5)),
                lWickPercent: parseFloat(lWickPercent),
                emaCutPosition: emaCutPosition,
                emaCutLongType: emaCutLongType,
                candlesSinceEmaCut: candlesSinceEmaCut,
                // New fields for consecutive EMA direction tracking
                UpConMediumEMA: upConMediumEMA,
                DownConMediumEMA: downConMediumEMA,
                UpConLongEMA: upConLongEMA,
                DownConLongEMA: downConLongEMA,
                // Placeholder fields
                isMark: "n",
                StatusCode: "",
                StatusDesc: seriesDesc,
                StatusDesc0: seriesDesc,
                hintStatus: "",
                suggestColor: "",
                winStatus: "",
                winCon: 0,
                lossCon: 0
            };

            this.analysisArray.push(analysisObj);
        }

        // Update nextColor for all items
        for (let i = 0; i <= this.analysisArray.length - 2; i++) {
            this.analysisArray[i].nextColor = this.analysisArray[i + 1].color;
        }

        return this.analysisArray;
    }

    /**
     * รับ summary statistics
     */
    getSummary() {
        if (this.analysisArray.length === 0) {
            return null;
        }

        const abnormalCount = this.analysisArray.filter(a => a.isAbnormalCandle).length;
        const abnormalATRCount = this.analysisArray.filter(a => a.isAbnormalATR).length;
        const greenCount = this.analysisArray.filter(a => a.color === 'Green').length;
        const redCount = this.analysisArray.filter(a => a.color === 'Red').length;
        const emaCrossoverCount = this.analysisArray.filter(a => a.emaCutLongType !== null).length;
        const upTrendCount = this.analysisArray.filter(a => a.emaCutLongType === 'UpTrend').length;
        const downTrendCount = this.analysisArray.filter(a => a.emaCutLongType === 'DownTrend').length;

        const latest = this.analysisArray[this.analysisArray.length - 1];

        return {
            totalCandles: this.analysisArray.length,
            greenCount: greenCount,
            redCount: redCount,
            abnormalCount: abnormalCount,
            abnormalATRCount: abnormalATRCount,
            emaCrossoverCount: emaCrossoverCount,
            upTrendCount: upTrendCount,
            downTrendCount: downTrendCount,
            latestCI: latest.choppyIndicator,
            latestADX: latest.adxValue,
            latestEmaShortDirection: latest.emaShortDirection,
            latestEmaMediumDirection: latest.emaMediumDirection,
            latestEmaLongDirection: latest.emaLongDirection,
            latestUpConMediumEMA: latest.UpConMediumEMA,
            latestDownConMediumEMA: latest.DownConMediumEMA,
            latestUpConLongEMA: latest.UpConLongEMA,
            latestDownConLongEMA: latest.DownConLongEMA
        };
    }

    /**
     * Export ข้อมูลเป็น JSON string
     */
    toJSON() {
        return JSON.stringify(this.analysisArray, null, 2);
    }
}

// Export for Node.js / CommonJS
if (typeof module !== 'undefined' && module.exports) {
    module.exports = AnalysisGenerator;
}
