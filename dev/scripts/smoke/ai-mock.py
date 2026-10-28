# A mock AI provider for the GUI smoke (steps 360-363): speaks OpenAI's chat completions on a free
# local port, written to argv[1]; every request body is appended to argv[2] (one JSON per line).
# No step ever calls a real provider.
import http.server, json, sys

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))))
        with open(sys.argv[2], "a") as f:
            f.write(json.dumps(body) + "\n")
        q = body["messages"][-1]["content"]
        if "slide deck" in q:
            a = "# Mock deck\n- First point\n# Second slide\n- Alpha\n- Beta"
        elif "podcast episode" in q:
            a = "Alex: Welcome to the show about the quick brown fox.\nSam: Thanks Alex, it jumps over the lazy dog."
        elif "Translate into Japanese" in q:
            a = "東京の素早い茶色の狐 (p. 1)"
        elif "Translate into" in q:
            a = "Le renard brun rapide (p. 1)"
        elif "main points" in q:
            a = "- Mockpoint the brown fox (p. 1)"
        elif "categor" in q:
            a = "Animals"
        else:
            a = "Mockanswer the fox jumps (p. 1)"
        if "Rate me" in q:
            out = b'{"error": {"message": "Rate limit reached"}}'
            self.send_response(429)
            self.send_header("retry-after", "3")
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)
            return
        if body.get("stream"):
            # Server-sent events, a few words per event, as OpenAI streams.
            words = a.split(" ")
            out = "".join("data: " + json.dumps({"choices": [{"delta": {"content": w + (" " if i < len(words) - 1 else "")}}]}) + "\n\n" for i, w in enumerate(words)) + "data: [DONE]\n\n"
            out = out.encode()
            self.send_response(200)
            self.send_header("content-type", "text/event-stream")
            self.send_header("content-length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)
            return
        out = json.dumps({"choices": [{"message": {"role": "assistant", "content": a}}]}).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *a):
        pass

server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
open(sys.argv[1], "w").write(str(server.server_address[1]))
server.serve_forever()
