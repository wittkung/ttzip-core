#!/usr/bin/env python3
# SPDX-License-Identifier: BSD-3-Clause OR Apache-2.0
#
# Copyright (c) 2026 Witt Kung <witt.w.kung@gmail.com>
# All rights reserved.
#
# TTZip: High-performance native archiving and compression engine.

# TTZip: Ed25519 Offline License Key Generator & Verification Utility

import argparse
import base64
import datetime
import json
import os
import sys
from cryptography.hazmat.primitives.asymmetric import ed25519

DEFAULT_KEY_DIR = os.path.expanduser("~/.ttzip_secrets")
PRIV_KEY_FILE = os.path.join(DEFAULT_KEY_DIR, "ed25519_license_priv.key")
PUB_KEY_FILE = os.path.join(DEFAULT_KEY_DIR, "ed25519_license_pub.key")

# Official Embedded Public Key for Test / Release
TEST_PUBLIC_KEY_BASE64 = "k8d2Y3x9eU+rC7v0A9B8dG2xL3h8dE4rS1v9B2c3dE4="

def generate_keypair():
    os.makedirs(DEFAULT_KEY_DIR, mode=0o700, exist_ok=True)
    private_key = ed25519.Ed25519PrivateKey.generate()
    public_key = private_key.public_key()
    
    priv_bytes = private_key.private_bytes_raw()
    pub_bytes = public_key.public_bytes_raw()
    
    with open(PRIV_KEY_FILE, "wb") as f:
        f.write(priv_bytes)
    os.chmod(PRIV_KEY_FILE, 0o600)
    
    with open(PUB_KEY_FILE, "wb") as f:
        f.write(pub_bytes)
        
    print(f"✅ Generated Ed25519 Keypair:")
    print(f"   Private Key: {PRIV_KEY_FILE}")
    print(f"   Public Key : {PUB_KEY_FILE}")
    print(f"   Public Key (Base64): {base64.b64encode(pub_bytes).decode('ascii')}")

def get_or_create_private_key() -> ed25519.Ed25519PrivateKey:
    if os.path.exists(PRIV_KEY_FILE):
        with open(PRIV_KEY_FILE, "rb") as f:
            return ed25519.Ed25519PrivateKey.from_private_bytes(f.read())
    else:
        # Fallback to deterministic key for local CI test vectors
        seed = b"TTZip-Ed25519-Deterministic-2026"  # 32 bytes
        return ed25519.Ed25519PrivateKey.from_private_bytes(seed)

def issue_license(email: str, order_id: str, tier: str = "pro_lifetime") -> str:
    private_key = get_or_create_private_key()
    public_key = private_key.public_key()
    pub_b64 = base64.b64encode(public_key.public_bytes_raw()).decode('ascii')
    
    payload = {
        "v": 1,
        "email": email,
        "tier": tier,
        "issued_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "order_id": order_id
    }
    
    payload_json = json.dumps(payload, separators=(',', ':'), sort_keys=True)
    payload_bytes = payload_json.encode('utf-8')
    signature_bytes = private_key.sign(payload_bytes)
    
    b64_payload = base64.b64encode(payload_bytes).decode('ascii')
    b64_sig = base64.b64encode(signature_bytes).decode('ascii')
    
    license_key = f"TTZIP1-{b64_payload}.{b64_sig}"
    return license_key, pub_b64

def verify_license(license_key: str, public_key_b64: str) -> bool:
    try:
        parts = license_key.strip().split(".")
        if len(parts) != 2 or not parts[0].startswith("TTZIP1-"):
            return False
        
        b64_payload = parts[0][len("TTZIP1-"):]
        b64_sig = parts[1]
        
        payload_bytes = base64.b64decode(b64_payload)
        signature_bytes = base64.b64decode(b64_sig)
        pub_bytes = base64.b64decode(public_key_b64)
        
        public_key = ed25519.Ed25519PublicKey.from_public_bytes(pub_bytes)
        public_key.verify(signature_bytes, payload_bytes)
        
        payload = json.loads(payload_bytes.decode('utf-8'))
        print(f"✓ Valid License for: {payload.get('email')} (Order: {payload.get('order_id')})")
        return True
    except Exception as e:
        print(f"✗ Verification Failed: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description="TTZip Ed25519 License Key Utility")
    subparsers = parser.add_subparsers(dest="command")
    
    # generate-keypair
    subparsers.add_parser("generate-keypair", help="Generate new Ed25519 keypair")
    
    # issue
    issue_parser = subparsers.add_parser("issue", help="Issue signed license key")
    issue_parser.add_argument("--email", required=True, help="Licensee email address")
    issue_parser.add_argument("--order", required=True, help="Order identifier")
    issue_parser.add_argument("--tier", default="pro_lifetime", choices=["pro_lifetime", "pro_business"], help="License tier")
    
    # verify
    verify_parser = subparsers.add_parser("verify", help="Verify a license key")
    verify_parser.add_argument("--key", required=True, help="License key string")
    verify_parser.add_argument("--pubkey", default="", help="Public key (Base64)")
    
    args = parser.parse_args()
    
    if args.command == "generate-keypair":
        generate_keypair()
    elif args.command == "issue":
        key, pub_b64 = issue_license(args.email, args.order, args.tier)
        print("======================================================================")
        print("🔑 TTZip Ed25519 Signed License Key")
        print("======================================================================")
        print(f"License Key : {key}")
        print(f"Public Key  : {pub_b64}")
        print("======================================================================")
    elif args.command == "verify":
        pubkey = args.pubkey
        if not pubkey:
            priv = get_or_create_private_key()
            pubkey = base64.b64encode(priv.public_key().public_bytes_raw()).decode('ascii')
        verify_license(args.key, pubkey)
    else:
        parser.print_help()

if __name__ == "__main__":
    main()
