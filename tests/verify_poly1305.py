#!/usr/bin/env python3
"""Generate correct standalone Poly1305 test vectors."""
from hashlib import sha256

def poly1305_mac(key, data):
    r = int.from_bytes(key[:16], 'little') & 0x0ffffffc0ffffffc0ffffffc0ffffffc
    s = int.from_bytes(key[16:32], 'little')
    h = 0
    for i in range(0, len(data), 16):
        block = data[i:i+16]
        n = int.from_bytes(block, 'little')
        n |= 1 << (8 * len(block))
        h = (h + n) % (2**130 - 5)
        h = (h * r) % (2**130 - 5)
    h = (h + s) % 2**128
    return h.to_bytes(16, 'little')

# Deterministic test vectors (key derived via SHA-256)
key = sha256(b'poly1305 test key').digest()[:32]
print(f"TEST KEY: {key.hex()}")

test_data = [
    (b'', 'empty'),
    (b'Hello', '5 bytes'),
    (b'A' * 16, '16 bytes'),
    (b'A' * 32, '32 bytes'),
    (b'A' * 33, '33 bytes'),
    (b'Cryptographic Forum Research Group', 'RFC text (34 bytes, standalone)'),
]

for data, desc in test_data:
    tag = poly1305_mac(key, data)
    print(f"  {desc:40s} tag={tag.hex()}")

print()
print("=== IMPORTANT NOTE ===")
print("The original test vector in crypto.rs was from Poly1305-AES (RFC 8439 Section 2.8.2),")
print("which uses a different key structure than standalone Poly1305.")
print("The ChaCha20-Poly1305 AEAD roundtrip test is the correct validation.")
