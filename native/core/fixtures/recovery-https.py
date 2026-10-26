"""Synthetic loopback protocol fixture. NOT NativeSync or model acceptance.
Accept exactly one synthetic message, then drop its HTTP ACK. Only request
counters are written; no Cookie, payload, message, or receipt logs/files.
"""
import hashlib
import http.server
import json
from pathlib import Path
import socket
import socketserver
import ssl
import sys

mode, ready_file, count_file = sys.argv[1:]
counts = {"post": 0, "session": 0, "history": 0, "other": 0}
payload = {"requestId": "lost_ack_123", "text": "synthetic ACK loss 🧭"}
receipt = None


def counted(kind):
    counts[kind] += 1
    Path(count_file).write_text(json.dumps(counts))


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def response(self, status, document, headers=None):
        body = json.dumps(document, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "text/html" if mode == "bad_content_type" and counts["history"] else "application/json")
        self.send_header("Content-Length", str(len(body)))
        for name, value in (headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)

    def authenticated(self):
        return (self.headers.get("Cookie") == "__Secure-vcp_mobile=" + "B" * 43
                and self.headers.get("Authorization") is None)

    def do_POST(self):
        global receipt
        counted("post")
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if 0 < length <= 1024 else b""
        valid = (self.path == "/mobile/api/conversations/topic_a/messages"
                 and self.authenticated()
                 and self.headers.get("Origin") == f"https://recovery.example.test:{self.server.server_port}"
                 and self.headers.get("Content-Type") == "application/json"
                 and self.headers.get("X-NativeSync-Entry-Id") == "a" * 64
                 and self.headers.get("X-NativeSync-Session-Marker") == "b" * 64
                 and json.loads(body or b"{}") == payload)
        if not valid or counts["post"] != 1:
            self.response(400, {})
            return
        canonical = {"topicKey": "topic_a", "kind": "message", "payload": dict(payload, attachments=[])}
        digest = hashlib.sha256(json.dumps(canonical, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()
        receipt = {"id": "exec_synthetic", "topicKey": "topic_a", "requestId": payload["requestId"], "state": "running",
                   "recovery": {"version": 1, "sessionMarker": "b" * 64, "payloadHash": digest, "kind": "message"}}
        # Accepted into this ephemeral fixture, but no response reaches client.
        self.connection.shutdown(socket.SHUT_RDWR)
        self.connection.close()

    def do_GET(self):
        if not self.authenticated() or self.headers.get("Origin") is not None:
            counted("other")
            self.response(400, {})
            return
        if self.path == "/mobile/api/session":
            counted("session")
            if mode in ("unauthenticated", "forbidden"):
                self.response(401 if mode == "unauthenticated" else 403, {})
                return
            if mode == "redirect":
                self.response(302, {}, {"Location": "/must-not-follow"})
                return
            headers = {"Set-Cookie": "__Secure-vcp_mobile=unexpected"} if mode == "cookie_rotation" else None
            self.response(200, {"authenticated": True,
                               "recovery": {"version": 1, "entryId": ("d" if mode == "entry_changed" else "a") * 64,
                                            "sessionMarker": ("c" if mode.startswith("cross_") else "b") * 64},
                               "topics": [{"key": "topic_b"}] if mode == "topic_revoked" else [{"key": "topic_a"}, {"key": "topic_b"}],
                               # Recovery must query original topic_a, not selected topic_b.
                               "selectedTopic": "topic_b"}, headers)
            return
        if self.path != "/mobile/api/conversations/topic_a?after=0":
            counted("other")
            self.response(404, {})
            return
        counted("history")
        if mode in ("history_unauthenticated", "history_forbidden", "history_redirect"):
            status = {"history_unauthenticated": 401, "history_forbidden": 403, "history_redirect": 302}[mode]
            self.response(status, {}, {"Location": "/must-not-follow"} if status == 302 else None)
            return
        if mode == "drop_history":
            self.connection.shutdown(socket.SHUT_RDWR)
            self.connection.close()
            return
        jobs = [] if mode in ("absent", "cross_absent") else [receipt]
        if mode == "conflict":
            jobs[0]["recovery"]["payloadHash"] = "0" * 64
        if mode == "metadata":
            del jobs[0]["recovery"]
        if mode == "wrong_topic":
            jobs[0]["topicKey"] = "topic_b"
        if mode == "duplicate":
            jobs.append(receipt)
        document = {"messages": [], "next": 0, "hasMore": False, "executions": jobs,
                    "topicState": {"historyReady": True, "executable": True}}
        if mode == "malformed_page":
            # Even an exact receipt cannot legitimize a broken page.
            document["next"] = 1
        if mode == "paged":
            document.update(messages=[{"seq": i, "id": f"m_{i}", "deleted": False, "message": {"text": "synthetic"}}
                                      for i in range(1, 201)], next=200, hasMore=True)
        self.response(200, document)


class LoopbackServer(http.server.HTTPServer):
    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name = "recovery.example.test"
        self.server_port = self.server_address[1]


server = LoopbackServer(("127.0.0.1", 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain("leaf.pem", "leaf.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
Path(count_file).write_text(json.dumps(counts))
ready = Path(ready_file)
temporary = ready.with_suffix(".tmp")
temporary.write_text(str(server.server_port))
temporary.replace(ready)
server.serve_forever()
