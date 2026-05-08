#!/usr/bin/env python3
import argparse
import os
import ssl
import sys
import urllib.request
from pathlib import Path


def load_cert_info(cert_path: Path) -> None:
    info = ssl._ssl._test_decode_cert(str(cert_path))
    print(f"subject={info.get('subject')}")
    print(f"issuer={info.get('issuer')}")
    print(f"san={info.get('subjectAltName')}")
    print(f"notBefore={info.get('notBefore')}")
    print(f"notAfter={info.get('notAfter')}")


def request_url(url: str, cert_path: Path, token: str | None) -> None:
    ctx = ssl.create_default_context(cafile=str(cert_path))
    headers = {}
    if token:
        headers["Authorization"] = f"Bearer {token.strip()}"

    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req, context=ctx, timeout=5) as resp:
        body = resp.read(512).decode("utf-8", "replace")
        print(f"status={resp.status}")
        print(body)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("cert", help="CA/证书文件路径")
    parser.add_argument(
        "--url",
        default="https://127.0.0.1:19090/admin/v1/runtime/status",
        help="要验证的 URL",
    )
    parser.add_argument(
        "--token",
        default=os.getenv("WARPARSE_TOKEN", ""),
        help="Bearer Token，可选，也可通过 WARPARSE_TOKEN 提供",
    )
    args = parser.parse_args()

    cert_path = Path(args.cert).expanduser().resolve()
    if not cert_path.exists():
        print(f"cert not found: {cert_path}", file=sys.stderr)
        return 2

    print(f"cert={cert_path}")
    load_cert_info(cert_path)

    try:
        request_url(args.url, cert_path, args.token or None)
        return 0
    except Exception as exc:
        print(f"request_error={exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
