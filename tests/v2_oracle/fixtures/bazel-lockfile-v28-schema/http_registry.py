from __future__ import annotations

import argparse
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Lock


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()
    log_lock = Lock()

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *handler_args, **handler_kwargs):
            super().__init__(*handler_args, directory=str(args.root), **handler_kwargs)

        def log_message(self, _format: str, *_values: object) -> None:
            pass

        def translate_path(self, path: str) -> str:
            if not path.startswith("/registry/"):
                return str(args.root / "__missing__")
            original = self.path
            self.path = path.removeprefix("/registry")
            try:
                return super().translate_path(self.path)
            finally:
                self.path = original

        def log_request(self, _code: int | str = "-", _size: int | str = "-") -> None:
            if "/modules/subject/" not in self.path:
                return
            with log_lock:
                with args.log.open("a", encoding="utf-8") as log:
                    log.write(json.dumps({"path": self.path}) + "\n")

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"http://127.0.0.1:{server.server_port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
