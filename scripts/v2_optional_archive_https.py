"""Serve a mutable omitted-checksum archive to one exact native Core test."""

from http.server import BaseHTTPRequestHandler, HTTPServer
import os
from pathlib import Path
import ssl
import subprocess
import sys
import tempfile
from threading import Thread


TEST = (
    "runtime::dice::repository_workspace_driver::tests::optional_archive_tests::"
    "native_original_optional_https_archive_is_fresh_across_a_b_a"
)
ARCHIVE = "source.tar.gz"


def make_tls(root):
    def openssl(*args):
        subprocess.run(["openssl", *args], cwd=root, check=True, timeout=10)

    key = ("-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:prime256v1", "-nodes")
    openssl(
        "req", "-x509", *key, "-subj", "/CN=Slug Archive Test CA", "-days", "1",
        "-keyout", "ca.key", "-out", "ca.pem",
        "-addext", "basicConstraints=critical,CA:TRUE",
        "-addext", "keyUsage=critical,keyCertSign",
    )
    openssl("req", *key, "-subj", "/CN=localhost", "-keyout", "leaf.key", "-out", "leaf.csr")
    (root / "leaf.ext").write_text("subjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\n")
    openssl(
        "x509", "-req", "-in", "leaf.csr", "-CA", "ca.pem", "-CAkey", "ca.key",
        "-set_serial", "1", "-days", "1", "-extfile", "leaf.ext", "-out", "leaf.pem",
    )
    tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    tls.load_cert_chain(root / "leaf.pem", root / "leaf.key")
    return tls


def run_fixture(root, executable):
    tls = make_tls(root)
    requests = 0
    count_path = root / "requests"
    count_path.write_text("0")

    class Server(HTTPServer):
        def get_request(self):
            socket, address = super().get_request()
            try:
                socket.settimeout(5)
                return tls.wrap_socket(socket, server_side=True), address
            except OSError:
                socket.close()
                raise

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            nonlocal requests
            if self.path != f"/{ARCHIVE}":
                self.send_error(404)
                return
            requests += 1
            count_path.write_text(str(requests))
            with (root / ARCHIVE).open("rb") as archive:
                body = archive.read(4097)
            if requests > 4 or not 0 < len(body) <= 4096:
                self.send_error(500)
                return
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    with Server(("127.0.0.1", 0), Handler) as server:
        host, port = server.server_address
        (root / "url").write_text(f"https://{host}:{port}/{ARCHIVE}")
        env = dict(
            os.environ, SSL_CERT_FILE=str(root / "ca.pem"), SLUG_TEST_ARCHIVE_HTTPS_DIR=str(root),
        )
        env.pop("SSL_CERT_DIR", None)
        thread = Thread(target=server.serve_forever, kwargs={"poll_interval": 0.05})
        thread.start()
        try:
            result = subprocess.run(
                [executable, "--ignored", "--exact", TEST], env=env, timeout=120,
            )
        finally:
            server.shutdown()
            thread.join()
        if result.returncode:
            return result.returncode
        if requests != 4:
            raise RuntimeError(f"expected 4 HTTPS GETs, observed {requests}")
        print(f"scoped HTTPS archive GETs {requests}")
        return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <Core test executable>")
    if not os.access(os.environ.get("SLUG_V2_BIN", ""), os.X_OK):
        sys.exit("set SLUG_V2_BIN to the compiled Slug CLI")
    with tempfile.TemporaryDirectory(prefix="slug-archive-https-") as tmp:
        sys.exit(run_fixture(Path(tmp).resolve(), Path(sys.argv[1]).resolve()))
