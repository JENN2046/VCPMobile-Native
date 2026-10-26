"""Ephemeral loopback TLS protocol fixture, not a NativeSync service or acceptance.
Only a request counter is written. Never logs bodies or Cookie values.
"""
import http.server
import json
from pathlib import Path
import socket
import ssl
import sys

mode, ready_file, count_file = sys.argv[1:]
count = 0

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        global count
        count += 1
        Path(count_file).write_text(str(count))
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if 0 < length <= 128 else b""
        valid = (self.path == "/mobile/api/pair"
                 and self.headers.get("Origin") == f"https://pairing.example.test:{self.server.server_port}"
                 and self.headers.get("Content-Type") == "application/json"
                 and self.headers.get("Cookie") is None
                 and self.headers.get("Authorization") is None
                 and json.loads(body or b"{}") == {"code": "a" * 32})
        if not valid:
            self.send_error(400)
            return
        if mode == "dropped":
            self.connection.shutdown(socket.SHUT_RDWR)
            self.connection.close()
            return
        status = 401 if mode == "rejected" else 302 if mode == "redirect" else 200
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        if mode == "redirect":
            self.send_header("Location", "https://redirect.example.test/mobile/api/pair")
        if mode in ("success", "duplicate"):
            cookie = "__Secure-vcp_mobile=" + "B" * 43 + "; Path=/mobile; HttpOnly; Secure; SameSite=Strict; Max-Age=600"
            self.send_header("Set-Cookie", cookie)
            if mode == "duplicate":
                self.send_header("Set-Cookie", cookie)
        response = json.dumps({"authenticated": status == 200}).encode()
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)

server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain("leaf.pem", "leaf.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
Path(ready_file).write_text(str(server.server_port))
server.serve_forever()
