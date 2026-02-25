<?php
/**
 * JavaScript Obfuscator Pro - Process Handler
 */
session_start();

require_once 'JSObfuscator.php';

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    header('Location: index.php');
    exit;
}

// Get JavaScript code
$jsCode = isset($_POST['js_code']) ? $_POST['js_code'] : '';

if (empty(trim($jsCode))) {
    header('Location: index.php?error=empty');
    exit;
}

// Store original size
$_SESSION['original_size'] = strlen($jsCode);

// Parse allowed domains
$allowedDomains = [];
if (!empty($_POST['allowed_domains'])) {
    $allowedDomains = array_map('trim', explode(',', $_POST['allowed_domains']));
    $allowedDomains = array_filter($allowedDomains);
}

// Build options array
$options = [
    // Anti-Debugging
    'disable_console' => isset($_POST['disable_console']),
    'detect_devtools' => isset($_POST['detect_devtools']),
    'debugger_trap' => isset($_POST['debugger_trap']),
    'timing_detection' => isset($_POST['timing_detection']),
    
    // Domain Lock
    'domain_lock' => isset($_POST['domain_lock']),
    'allowed_domains' => $allowedDomains,
    'check_referrer' => isset($_POST['check_referrer']),
    'domain_encrypt' => isset($_POST['domain_encrypt']),
    
    // Obfuscation
    'name_mangling' => isset($_POST['name_mangling']),
    'string_encoding' => isset($_POST['string_encoding']),
    'control_flow' => isset($_POST['control_flow']),
    'dead_code' => isset($_POST['dead_code']),
    'self_defending' => isset($_POST['self_defending']),
    
    // Advanced
    'encoding_type' => isset($_POST['encoding_type']) ? $_POST['encoding_type'] : 'hex',
    'obfuscation_level' => isset($_POST['obfuscation_level']) ? $_POST['obfuscation_level'] : 'medium',
    'minify' => isset($_POST['minify']),
    'add_wrapper' => isset($_POST['add_wrapper']),
];

try {
    // Create obfuscator and process
    $obfuscator = new JSObfuscator($options);
    $obfuscatedCode = $obfuscator->obfuscate($jsCode);
    
    // Save to temp file for download
    file_put_contents('temp_output.js', $obfuscatedCode);
    
    // Also save options for reference
    $_SESSION['last_options'] = $options;
    
    // Redirect back with success
    header('Location: index.php?success=1');
    exit;
    
} catch (Exception $e) {
    // Handle error
    $_SESSION['error'] = $e->getMessage();
    header('Location: index.php?error=1');
    exit;
}
