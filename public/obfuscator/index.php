<!DOCTYPE html>
<html lang="th">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>JavaScript Obfuscator Pro</title>
    <link rel="stylesheet" href="style.css">
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Fira+Code&display=swap" rel="stylesheet">
</head>
<body>
    <div class="container">
        <header class="header">
            <div class="logo">
                <span class="logo-icon">🔐</span>
                <h1>JavaScript Obfuscator Pro</h1>
            </div>
            <p class="tagline">ปกป้อง JavaScript ของคุณด้วยเทคนิคขั้นสูง</p>
        </header>

        <form method="POST" action="process.php" class="main-form">
            <!-- Input Section -->
            <div class="section input-section">
                <div class="section-header">
                    <h2><span class="icon">📝</span> JavaScript Input</h2>
                    <div class="actions">
                        <button type="button" class="btn btn-sm" onclick="clearInput()">Clear</button>
                        <button type="button" class="btn btn-sm" onclick="loadSample()">Sample Code</button>
                    </div>
                </div>
                <textarea name="js_code" id="js_code" placeholder="// Paste your JavaScript code here...
function myFunction() {
    console.log('Hello World');
}" required><?php echo isset($_POST['js_code']) ? htmlspecialchars($_POST['js_code']) : ''; ?></textarea>
            </div>

            <!-- Options Grid -->
            <div class="options-grid">
                <!-- Anti-Debugging Options -->
                <div class="option-card">
                    <div class="card-header">
                        <span class="card-icon">🚫</span>
                        <h3>Anti-Debugging</h3>
                    </div>
                    <div class="card-body">
                        <label class="checkbox-item">
                            <input type="checkbox" name="disable_console" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">Override Console Object</span>
                            <span class="tooltip" data-tip="ปิดการใช้งาน console.log, warn, error">?</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="detect_devtools" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">Detect DevTools</span>
                            <span class="tooltip" data-tip="ตรวจจับการเปิด DevTools">?</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="debugger_trap" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">Debugger Trap</span>
                            <span class="tooltip" data-tip="ใส่ debugger loop ป้องกันการ debug">?</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="timing_detection" value="1">
                            <span class="checkmark"></span>
                            <span class="label-text">Timing Detection</span>
                            <span class="tooltip" data-tip="ตรวจจับ breakpoint จากเวลาที่ช้า">?</span>
                        </label>
                    </div>
                </div>

                <!-- Domain Lock Options -->
                <div class="option-card">
                    <div class="card-header">
                        <span class="card-icon">🌐</span>
                        <h3>Domain Lock</h3>
                    </div>
                    <div class="card-body">
                        <label class="checkbox-item">
                            <input type="checkbox" name="domain_lock" value="1" id="domain_lock_checkbox">
                            <span class="checkmark"></span>
                            <span class="label-text">Enable Domain Lock</span>
                        </label>
                        <div class="input-group">
                            <label>Allowed Domains (comma separated)</label>
                            <input type="text" name="allowed_domains" id="allowed_domains" 
                                   placeholder="example.com, app.example.com" 
                                   value="<?php echo isset($_POST['allowed_domains']) ? htmlspecialchars($_POST['allowed_domains']) : ''; ?>">
                        </div>
                        <label class="checkbox-item">
                            <input type="checkbox" name="check_referrer" value="1">
                            <span class="checkmark"></span>
                            <span class="label-text">Check Referrer</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="domain_encrypt" value="1">
                            <span class="checkmark"></span>
                            <span class="label-text">Domain-based Encryption</span>
                            <span class="tooltip" data-tip="ใช้ domain เป็น key decrypt">?</span>
                        </label>
                    </div>
                </div>

                <!-- Obfuscation Options -->
                <div class="option-card">
                    <div class="card-header">
                        <span class="card-icon">🔒</span>
                        <h3>Obfuscation</h3>
                    </div>
                    <div class="card-body">
                        <label class="checkbox-item">
                            <input type="checkbox" name="name_mangling" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">Variable Name Mangling</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="string_encoding" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">String Encoding (Hex/Base64)</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="control_flow" value="1">
                            <span class="checkmark"></span>
                            <span class="label-text">Control Flow Flattening</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="dead_code" value="1">
                            <span class="checkmark"></span>
                            <span class="label-text">Dead Code Injection</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="self_defending" value="1">
                            <span class="checkmark"></span>
                            <span class="label-text">Self-Defending Code</span>
                            <span class="tooltip" data-tip="ตรวจจับ beautify แล้วพัง">?</span>
                        </label>
                    </div>
                </div>

                <!-- Advanced Options -->
                <div class="option-card">
                    <div class="card-header">
                        <span class="card-icon">⚙️</span>
                        <h3>Advanced Settings</h3>
                    </div>
                    <div class="card-body">
                        <div class="input-group">
                            <label>Encoding Type</label>
                            <select name="encoding_type">
                                <option value="hex">Hexadecimal</option>
                                <option value="base64">Base64</option>
                                <option value="unicode">Unicode Escape</option>
                                <option value="mixed">Mixed Encoding</option>
                            </select>
                        </div>
                        <div class="input-group">
                            <label>Obfuscation Level</label>
                            <select name="obfuscation_level">
                                <option value="light">Light (Faster)</option>
                                <option value="medium" selected>Medium (Balanced)</option>
                                <option value="heavy">Heavy (Maximum Protection)</option>
                            </select>
                        </div>
                        <label class="checkbox-item">
                            <input type="checkbox" name="minify" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">Minify Output</span>
                        </label>
                        <label class="checkbox-item">
                            <input type="checkbox" name="add_wrapper" value="1" checked>
                            <span class="checkmark"></span>
                            <span class="label-text">Add IIFE Wrapper</span>
                        </label>
                    </div>
                </div>
            </div>

            <!-- Submit Button -->
            <div class="submit-section">
                <button type="submit" class="btn btn-primary btn-lg">
                    <span class="btn-icon">🔐</span>
                    Obfuscate JavaScript
                </button>
            </div>
        </form>

        <?php if (isset($_GET['success']) && file_exists('temp_output.js')): ?>
        <!-- Output Section -->
        <div class="section output-section">
            <div class="section-header">
                <h2><span class="icon">✅</span> Obfuscated Output</h2>
                <div class="actions">
                    <button type="button" class="btn btn-sm btn-success" onclick="copyOutput()">
                        <span>📋</span> Copy
                    </button>
                    <a href="download.php" class="btn btn-sm btn-primary">
                        <span>⬇️</span> Download
                    </a>
                </div>
            </div>
            <textarea id="output_code" readonly><?php echo htmlspecialchars(file_get_contents('temp_output.js')); ?></textarea>
            
            <!-- Stats -->
            <div class="stats-bar">
                <?php 
                $original_size = isset($_SESSION['original_size']) ? $_SESSION['original_size'] : 0;
                $obfuscated_size = filesize('temp_output.js');
                ?>
                <div class="stat">
                    <span class="stat-label">Original Size:</span>
                    <span class="stat-value"><?php echo number_format($original_size); ?> bytes</span>
                </div>
                <div class="stat">
                    <span class="stat-label">Obfuscated Size:</span>
                    <span class="stat-value"><?php echo number_format($obfuscated_size); ?> bytes</span>
                </div>
                <div class="stat">
                    <span class="stat-label">Protection Applied:</span>
                    <span class="stat-value stat-success">✓ Complete</span>
                </div>
            </div>
        </div>
        <?php endif; ?>

        <!-- Features Section -->
        <div class="features-section">
            <h2>🛡️ Protection Features</h2>
            <div class="features-grid">
                <div class="feature">
                    <div class="feature-icon">🚫</div>
                    <h4>Anti-Debugging</h4>
                    <p>ป้องกันการใช้ DevTools และ console</p>
                </div>
                <div class="feature">
                    <div class="feature-icon">🌐</div>
                    <h4>Domain Lock</h4>
                    <p>จำกัดให้ทำงานเฉพาะ domain ที่อนุญาต</p>
                </div>
                <div class="feature">
                    <div class="feature-icon">🔒</div>
                    <h4>Code Obfuscation</h4>
                    <p>เปลี่ยนชื่อตัวแปร, encode strings</p>
                </div>
                <div class="feature">
                    <div class="feature-icon">⚡</div>
                    <h4>Self-Defending</h4>
                    <p>ตรวจจับการ beautify และหยุดทำงาน</p>
                </div>
            </div>
        </div>

        <footer class="footer">
            <p>JavaScript Obfuscator Pro © 2026 | Built with ❤️</p>
        </footer>
    </div>

    <script>
        function clearInput() {
            document.getElementById('js_code').value = '';
        }

        function loadSample() {
            document.getElementById('js_code').value = `// Sample JavaScript Code
function calculateTotal(items) {
    let total = 0;
    const taxRate = 0.07;
    
    for (let i = 0; i < items.length; i++) {
        total += items[i].price * items[i].quantity;
    }
    
    const tax = total * taxRate;
    const grandTotal = total + tax;
    
    console.log('Subtotal:', total);
    console.log('Tax:', tax);
    console.log('Grand Total:', grandTotal);
    
    return grandTotal;
}

const myItems = [
    { name: 'Apple', price: 1.5, quantity: 4 },
    { name: 'Banana', price: 0.75, quantity: 6 },
    { name: 'Orange', price: 2.0, quantity: 3 }
];

const result = calculateTotal(myItems);
document.getElementById('result').textContent = 'Total: $' + result.toFixed(2);`;
        }

        function copyOutput() {
            const output = document.getElementById('output_code');
            output.select();
            document.execCommand('copy');
            
            // Show feedback
            const btn = event.target.closest('button');
            const originalText = btn.innerHTML;
            btn.innerHTML = '<span>✓</span> Copied!';
            btn.classList.add('btn-copied');
            
            setTimeout(() => {
                btn.innerHTML = originalText;
                btn.classList.remove('btn-copied');
            }, 2000);
        }

        // Toggle domain input based on checkbox
        document.getElementById('domain_lock_checkbox').addEventListener('change', function() {
            document.getElementById('allowed_domains').disabled = !this.checked;
        });
    </script>
</body>
</html>
