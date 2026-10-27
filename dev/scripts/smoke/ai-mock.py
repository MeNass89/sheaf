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
        elif "Translate into" in q:
            a = "Le renard brun rapide (p. 1)"
        elif "main points" in q:
            a = "- Mockpoint the brown fox (p. 1)"
        elif "categor" in q:
            a = "Animals"
        else:
            a = "Mockanswer the fox jumps (p. 1)"
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
