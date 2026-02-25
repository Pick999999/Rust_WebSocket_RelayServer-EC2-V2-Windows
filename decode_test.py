import os
import sys

root = r"d:/Rust/RustAndSocket/Rust_WebSocket_RelayServer-EC2-V2-Windows/backup"

def check_decode(name):
    try:
        # Strategy: The filename string is formed by interpreting the original UTF-8 bytes 
        # using a single-byte encoding (likely CP1252 or a mix where Thai bytes map to CP874 chars).
        # We need to recover the original bytes.
        
        # In this specific corruption case (UTF-8 bytes displayed as TIS-620/CP874/CP1252):
        # 0xE0 map to 'เ'
        # 0xB9 map to 'น'
        # 0x80 map to '€'
        
        # We iterate characters and recover bytes.
        
        recovered_bytes = bytearray()
        
        for char in name:
            # First try encoding as CP874 (Thai)
            try:
                b = char.encode('cp874')
                if len(b) == 1:
                    recovered_bytes.append(b[0])
                    continue
            except:
                pass
                
            # Then try CP1252 (Western) - for things like €
            try:
                b = char.encode('cp1252')
                if len(b) == 1:
                    recovered_bytes.append(b[0])
                    continue
            except:
                pass
            
            # Fallback: if it's ascii, just take it
            if ord(char) < 128:
                recovered_bytes.append(ord(char))
                continue
            
            # If we get here, we couldn't map it.
            # print(f"Warning: Could not map char {char!r} in {name}")
            # Just append a placeholder or try to guess?
            # For now let's hope the above covers it.
            return None # Fail

        return recovered_bytes.decode('utf-8')
    except Exception as e:
        # print(f"Error decoding {name}: {e}")
        return None

if not os.path.exists(root):
    print(f"Directory not found: {root}")
    sys.exit(1)

files = os.listdir(root)
print(f"Found {len(files)} files/folders.")

for f in files:
    decoded = check_decode(f)
    if decoded:
        print(f"ORIG: {f}")
        print(f"NEW : {decoded}")
        print("---")
    else:
        print(f"SKIP: {f} (Could not decode)")
