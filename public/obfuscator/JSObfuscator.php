<?php
/**
 * JavaScript Obfuscator Pro - Core Class
 */
class JSObfuscator {
    private $options = [];
    private $variableMap = [];
    private $stringArrayName = '';
    private $counter = 0;
    private $originalFunctionNames = []; // Store original function names before mangling
    
    private $reservedWords = [
        'break', 'case', 'catch', 'continue', 'debugger', 'default', 'delete',
        'do', 'else', 'finally', 'for', 'function', 'if', 'in', 'instanceof',
        'new', 'return', 'switch', 'this', 'throw', 'try', 'typeof', 'var',
        'void', 'while', 'with', 'class', 'const', 'let', 'true', 'false', 
        'null', 'undefined', 'NaN', 'window', 'document', 'console', 'Math',
        'Date', 'Array', 'Object', 'String', 'Number', 'JSON', 'Promise',
        'async', 'await', 'of', 'get', 'set', 'arguments', 'eval'
    ];

    public function __construct($options = []) {
        $this->options = array_merge([
            'disable_console' => false,
            'detect_devtools' => false,
            'debugger_trap' => false,
            'timing_detection' => false,
            'domain_lock' => false,
            'allowed_domains' => [],
            'check_referrer' => false,
            'domain_encrypt' => false,
            'name_mangling' => false,
            'string_encoding' => false,
            'control_flow' => false,
            'dead_code' => false,
            'self_defending' => false,
            'encoding_type' => 'hex',
            'obfuscation_level' => 'medium',
            'minify' => true,
            'add_wrapper' => true,
        ], $options);
        
        $this->stringArrayName = $this->generateVarName();
    }

    public function obfuscate($code) {
        $result = $code;
        
        // IMPORTANT: Extract original function names BEFORE any transformation
        $this->extractFunctionNames($code);
        
        if ($this->options['string_encoding']) {
            $result = $this->encodeStrings($result);
        }
        if ($this->options['name_mangling']) {
            $result = $this->mangleNames($result);
        }
        if ($this->options['control_flow']) {
            $result = $this->flattenControlFlow($result);
        }
        if ($this->options['dead_code']) {
            $result = $this->injectDeadCode($result);
        }
        
        $protectionCode = $this->buildProtectionCode();
        
        if ($this->options['add_wrapper']) {
            $result = $this->wrapInIIFE($result, $protectionCode);
        } else {
            $result = $protectionCode . "\n" . $result;
        }
        
        if ($this->options['self_defending']) {
            $result = $this->addSelfDefending($result);
        }
        if ($this->options['minify']) {
            $result = $this->minify($result);
        }
        
        return $result;
    }

    private function generateVarName() {
        $prefix = '_0x';
        $chars = '0123456789abcdef';
        $name = $prefix;
        for ($i = 0; $i < 6; $i++) {
            $name .= $chars[rand(0, 15)];
        }
        return $name . $this->counter++;
    }

    private function encodeStrings($code) {
        $stringArray = [];
        $arrayVarName = $this->stringArrayName;
        
        $pattern = '/([\'"])(?:\\\\.|(?!\1).)*\1/';
        
        $code = preg_replace_callback($pattern, function($matches) use (&$stringArray, $arrayVarName) {
            $content = substr($matches[0], 1, -1);
            if (strlen($content) < 2) return $matches[0];
            
            $encoded = $this->encodeString($content);
            $index = count($stringArray);
            $stringArray[] = $encoded;
            return $arrayVarName . '[' . $index . ']';
        }, $code);
        
        if (!empty($stringArray)) {
            $code = $this->buildStringArrayCode($stringArray, $arrayVarName) . "\n" . $code;
        }
        return $code;
    }

    private function encodeString($str) {
        $type = $this->options['encoding_type'];
        if ($type === 'mixed') {
            $types = ['hex', 'base64', 'unicode'];
            $type = $types[array_rand($types)];
        }
        switch ($type) {
            case 'base64': return 'b64:' . base64_encode($str);
            case 'unicode': return 'uni:' . $this->toUnicodeEscape($str);
            default: return 'hex:' . bin2hex($str);
        }
    }

    private function toUnicodeEscape($str) {
        $result = '';
        $len = mb_strlen($str, 'UTF-8');
        for ($i = 0; $i < $len; $i++) {
            $char = mb_substr($str, $i, 1, 'UTF-8');
            $result .= sprintf('%04x', mb_ord($char, 'UTF-8'));
        }
        return $result;
    }

    private function buildStringArrayCode($stringArray, $varName) {
        $encodedArray = json_encode($stringArray);
        // Fixed UTF-8 decoding: Use proper byte-to-UTF8 conversion for multi-byte characters (Thai, etc.)
        return "var {$varName}=(function(){var _a={$encodedArray};var _d=function(s){if(s.indexOf('hex:')===0){var h=s.slice(4),b=[];for(var i=0;i<h.length;i+=2)b.push(parseInt(h.substr(i,2),16));return new TextDecoder('utf-8').decode(new Uint8Array(b));}if(s.indexOf('b64:')===0){var bin=atob(s.slice(4)),b=new Uint8Array(bin.length);for(var i=0;i<bin.length;i++)b[i]=bin.charCodeAt(i);return new TextDecoder('utf-8').decode(b);}if(s.indexOf('uni:')===0){var u=s.slice(4),r='';for(var i=0;i<u.length;i+=4)r+=String.fromCodePoint(parseInt(u.substr(i,4),16));return r;}return s;};return _a.map(_d);})();";
    }

    private function mangleNames($code) {
        preg_match_all('/\b(var|let|const)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)/', $code, $m1);
        preg_match_all('/function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*\(/', $code, $m2);
        
        foreach (array_merge($m1[2], $m2[1]) as $name) {
            if (!in_array($name, $this->reservedWords) && !isset($this->variableMap[$name])) {
                $this->variableMap[$name] = $this->generateVarName();
            }
        }
        
        uksort($this->variableMap, fn($a, $b) => strlen($b) - strlen($a));
        foreach ($this->variableMap as $orig => $mang) {
            // Use negative lookbehind to NOT mangle property access (e.g., data.token, obj.name)
            // Only mangle standalone variable references, not property names after a dot
            $code = preg_replace('/(?<!\.)(?<!\?\.)(?<!\[\'|\[")\b' . preg_quote($orig, '/') . '\b(?!\s*:(?!=))/', $mang, $code);
        }
        return $code;
    }

    private function flattenControlFlow($code) {
        $sv = $this->generateVarName();
        $lv = $this->generateVarName();
        return preg_replace_callback('/function(\s*[a-zA-Z_$]*)\s*\(([^)]*)\)\s*\{([^{}]*(?:\{[^{}]*\}[^{}]*)*)\}/s', 
            fn($m) => "function{$m[1]}({$m[2]}){var {$sv}=0,{$lv}=true;while({$lv}){switch({$sv}){case 0:{$m[3]}{$lv}=false;break;}}}", $code);
    }

    private function injectDeadCode($code) {
        $dead = [
            'if(false){' . $this->generateVarName() . '=Math.random();}',
            'void function(){return ' . rand(1,999) . ';}();',
        ];
        $n = $this->options['obfuscation_level'] === 'light' ? 2 : ($this->options['obfuscation_level'] === 'heavy' ? 8 : 4);
        $lines = explode("\n", $code);
        for ($i = 0; $i < $n; $i++) {
            array_splice($lines, rand(0, count($lines)-1), 0, [$dead[array_rand($dead)]]);
        }
        return implode("\n", $lines);
    }

    private function buildProtectionCode() {
        $p = [];
        if ($this->options['disable_console']) $p[] = $this->getConsoleOverride();
        if ($this->options['detect_devtools']) $p[] = $this->getDevToolsDetection();
        if ($this->options['debugger_trap']) $p[] = $this->getDebuggerTrap();
        if ($this->options['timing_detection']) $p[] = $this->getTimingDetection();
        if ($this->options['domain_lock'] && !empty($this->options['allowed_domains'])) $p[] = $this->getDomainLock();
        if ($this->options['check_referrer'] && !empty($this->options['allowed_domains'])) $p[] = $this->getReferrerCheck();
        return implode("\n", $p);
    }

    private function getConsoleOverride() {
        return "(function(){var n=function(){};['log','warn','error','info','debug'].forEach(function(m){console[m]=n;});})();";
    }

    private function getDevToolsDetection() {
        return "(function(){setInterval(function(){if(window.outerWidth-window.innerWidth>160||window.outerHeight-window.innerHeight>160){document.body.innerHTML='';}},1000);})();";
    }

    private function getDebuggerTrap() {
        return "(function(){setInterval(function(){debugger;},100);})();";
    }

    private function getTimingDetection() {
        return "(function(){setInterval(function(){var s=performance.now();debugger;if(performance.now()-s>100)document.body.innerHTML='';},5000);})();";
    }

    private function getDomainLock() {
        $d = json_encode($this->options['allowed_domains']);
        return "(function(){var a={$d},h=location.hostname,v=false;for(var i=0;i<a.length;i++){if(h===a[i]||h.endsWith('.'+a[i])){v=true;break;}}if(!v){document.body.innerHTML='<h1>Access Denied</h1>';throw new Error('Domain error');}})();";
    }

    private function getReferrerCheck() {
        $d = json_encode($this->options['allowed_domains']);
        return "(function(){var a={$d},r=document.referrer;if(r){try{var h=new URL(r).hostname,v=false;for(var i=0;i<a.length;i++){if(h===a[i])v=true;}if(!v)console.warn('Referrer check failed');}catch(e){}}})();";
    }

    /**
     * Extract function names from original code BEFORE any transformation
     */
    private function extractFunctionNames($code) {
        $this->originalFunctionNames = [];
        
        // Match: function functionName(
        // Match: async function functionName(
        preg_match_all('/(?:^|\n)\s*(?:async\s+)?function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*\(/m', $code, $matches);
        if (!empty($matches[1])) {
            $this->originalFunctionNames = array_merge($this->originalFunctionNames, $matches[1]);
        }
        
        // Match: const/let/var functionName = function
        // Match: const/let/var functionName = async function  
        // Match: const/let/var functionName = () =>
        preg_match_all('/(?:^|\n)\s*(?:const|let|var)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*=\s*(?:async\s+)?(?:function|\([^)]*\)\s*=>)/m', $code, $matches2);
        if (!empty($matches2[1])) {
            $this->originalFunctionNames = array_merge($this->originalFunctionNames, $matches2[1]);
        }
        
        // Remove duplicates and filter out reserved words
        $this->originalFunctionNames = array_unique($this->originalFunctionNames);
        $this->originalFunctionNames = array_filter($this->originalFunctionNames, function($name) {
            return !in_array($name, $this->reservedWords);
        });
    }

    private function wrapInIIFE($code, $prot) {
        // Build export statements using ORIGINAL function names stored before mangling
        $exports = '';
        foreach ($this->originalFunctionNames as $originalName) {
            // Get mangled name if it exists, otherwise use original
            $mangledName = isset($this->variableMap[$originalName]) ? $this->variableMap[$originalName] : $originalName;
            // Export: window.originalName = mangledName;
            $exports .= "window.{$originalName}={$mangledName};";
        }
        
        return "(function(){'use strict';{$prot}\n{$code}\n{$exports}})();";
    }

    private function addSelfDefending($code) {
        $f = $this->generateVarName();
        return "(function(){var {$f}=function(){try{Function('return this')();}catch(e){while(true){}}};{$f}();})();\n{$code}";
    }

    private function minify($code) {
        $code = preg_replace('#/\*.*?\*/#s', '', $code);
        $code = preg_replace('#//.*?$#m', '', $code);
        $code = preg_replace('/\s+/', ' ', $code);
        $code = preg_replace('/\s*([{}\[\]();,:<>=!&|?+\-*\/])\s*/', '$1', $code);
        $code = preg_replace('/\b(var|let|const|function|return|if|for|while)\s+/', '$1 ', $code);
        return trim($code);
    }
}
