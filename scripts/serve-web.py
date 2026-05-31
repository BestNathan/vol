#!/usr/bin/env python3
"""Serve Dioxus WASM build output with cache headers.

Usage:
    python3 scripts/serve-web.py [DIR] [--port PORT]

For content-hashed assets (.wasm, .js, .css with hashes in filename):
    Cache-Control: public, max-age=31536000, immutable
For index.html:
    Cache-Control: no-cache
For everything else:
    Cache-Control: public, max-age=3600
"""

import http.server
import os
import re
import sys


_HASHED = re.compile(r'[-_][0-9a-f]{8,}\.')


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        self.directory = kwargs.pop("directory", None) or os.getcwd()
        super().__init__(*args, directory=self.directory, **kwargs)

    def end_headers(self):
        path = self.path.rsplit("?", 1)[0]
        basename = os.path.basename(path) if path else ""

        if basename == "index.html" or basename == "":
            self.send_header("Cache-Control", "no-cache")
        elif _HASHED.search(basename):
            self.send_header("Cache-Control", "public, max-age=31536000, immutable")
        else:
            self.send_header("Cache-Control", "public, max-age=3600")

        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        super().end_headers()


if __name__ == "__main__":
    serve_dir = sys.argv[1] if len(sys.argv) > 1 and not sys.argv[1].startswith("--") else "."
    port = 8080
    try:
        pi = sys.argv.index("--port")
        port = int(sys.argv[pi + 1])
    except (ValueError, IndexError):
        pass

    print(f"Serving {os.path.abspath(serve_dir)} on http://0.0.0.0:{port}")
    httpd = http.server.HTTPServer(
        ("0.0.0.0", port),
        lambda *a, **kw: Handler(*a, directory=serve_dir, **kw),
    )
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")
