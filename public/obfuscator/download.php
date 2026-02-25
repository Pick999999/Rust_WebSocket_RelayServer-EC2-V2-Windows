<?php
/**
 * JavaScript Obfuscator Pro - Download Handler
 */

$filename = 'temp_output.js';

if (!file_exists($filename)) {
    header('Location: index.php?error=nofile');
    exit;
}

// Set headers for download
header('Content-Type: application/javascript');
header('Content-Disposition: attachment; filename="obfuscated_' . date('Ymd_His') . '.js"');
header('Content-Length: ' . filesize($filename));
header('Cache-Control: no-cache, must-revalidate');
header('Expires: 0');

// Output file
readfile($filename);

// Optionally delete temp file after download
// unlink($filename);

exit;
