const body = document.body;
const sidebarTrigger = document.getElementById('sidebarTrigger');
const panelA = document.getElementById('panelA');
const closeSidebarBtn = document.getElementById('closeSidebar');
const menuToggleBtn = document.getElementById('menuToggle');

// 1. Function เพื่อปรับขนาด Chart (สำคัญมากสำหรับ Lightweight Charts)
function resizeChart() {
    // ต้องเรียก chart.resize()
    if (window.chart) {
        const chartContainer = document.getElementById('chartContainer');
        window.chart.resize(chartContainer.clientWidth, chartContainer.clientHeight);
    }
}

// 2. การจัดการ Sidebar (Trigger Zone)
const openSidebar = () => {
    if (!body.classList.contains('sidebar-open')) {
        body.classList.add('sidebar-open');
        setTimeout(resizeChart, 350); // หน่วงเวลาให้ CSS Transition ทำงานเสร็จก่อน
    }
};

const closeSidebar = () => {
    if (body.classList.contains('sidebar-open')) {
        body.classList.remove('sidebar-open');
        setTimeout(resizeChart, 350); // หน่วงเวลาให้ CSS Transition ทำงานเสร็จก่อน
    }
};

// Trigger เมื่อเมาส์เข้าสู่พื้นที่ซ้าย
sidebarTrigger.addEventListener('mouseenter', openSidebar);

// ปิดเมื่อกดปุ่ม X
closeSidebarBtn.addEventListener('click', closeSidebar);

// ปิดเมื่อเมาส์ออกจาก Sidebar
panelA.addEventListener('mouseleave', (e) => {
    // ตรวจสอบว่าเมาส์ไม่ได้อยู่ในพื้นที่ Trigger Zone (เผื่อกรณีเมาส์ยังอยู่ขอบๆ)
    // สำหรับการออกแบบนี้ การคลิกที่ปุ่ม X หรือการใช้ Menu Toggle จะควบคุมได้ดีกว่า
    // แต่ถ้าต้องการให้มันซ่อนอัตโนมัติเมื่อเมาส์ออกนอก Panel A:
     if (e.clientX > panelA.clientWidth) { // เช็คตำแหน่งเมาส์นอกขอบขวาของ Sidebar
         closeSidebar();
     }
});


// Menu Toggle Button (สำหรับมือถือ/ทางเลือก)
menuToggleBtn.addEventListener('click', () => {
    if (body.classList.contains('sidebar-open')) {
        closeSidebar();
    } else {
        openSidebar();
    }
});

// ******* INITIALIZE CHART (Simplified) *******
document.addEventListener('DOMContentLoaded', () => {
    const chartContainer = document.getElementById('chartContainer');

    // ตรวจสอบว่า LightweightCharts โหลดแล้ว
    if (window.LightweightCharts) {
        window.chart = LightweightCharts.create(chartContainer, {
            width: chartContainer.clientWidth,
            height: chartContainer.clientHeight,
            layout: {
                backgroundColor: 'var(--bg-card)',
                textColor: 'var(--text-light)',
            },
            grid: {
                vertLines: { color: '#3c3c50' },
                horzLines: { color: '#3c3c50' },
            },
            // ... อื่นๆ
        });

        // เพิ่ม Candlestick Series และ Data (ตามที่เคยนำเสนอ)
        const series = window.chart.addCandlestickSeries();
        series.setData([ /* Sample Data */ ]);

        window.addEventListener('resize', resizeChart);
    }
});