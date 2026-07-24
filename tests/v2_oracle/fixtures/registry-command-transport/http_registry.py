from __future__ import annotations

import argparse
import json
from threading import Lock
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()
    log_lock = Lock()

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *handler_args, **kwargs):
            super().__init__(*handler_args, directory=str(args.root), **kwargs)

        def do_GET(self) -> None:
            if self.path.startswith("/fatal/"):
                self.send_error(500, "fixture fatal registry response")
                return
            super().do_GET()

        def log_message(self, _format: str, *values: object) -> None:
            path = urlsplit(self.path).path
            if not (
                path.endswith("/bazel_registry.json")
                or "/modules/yyy/" in path
                or "/modules/missingprobe/" in path
                or "/modules/fatalprobe/" in path
            ):
                return
            with log_lock, args.log.open("a", encoding="utf-8") as log:
                log.write(json.dumps({"path": path}, sort_keys=True) + "\n")

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"http://127.0.0.1:{server.server_port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
