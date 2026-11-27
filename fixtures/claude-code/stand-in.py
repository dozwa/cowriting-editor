#!/usr/bin/env python3
"""Stands in for `claude` in the tests: takes the arguments the editor starts Claude Code with,
talks to the editor's MCP server as Claude Code does, and prints events in its stream format.
What it does is chosen by a word in the question."""

import json
import sys
import urllib.request

args = sys.argv[1:]
if args == ["--version"]:
    print("1.0.123 (Claude Code)")
    sys.exit(0)
after = lambda flag: args[args.index(flag) + 1] if flag in args else None
question = sys.stdin.read()
system = open(after("--system-prompt-file"), encoding="utf-8").read()
emit = lambda event: print(json.dumps(event), flush=True)
config = json.loads(after("--mcp-config") or "{}").get("mcpServers", {}).get("cowriting")


def mcp(method, params=None, call_id=1):
    body = {"jsonrpc": "2.0", "method": method, "params": params or {}}
    if call_id is not None:
        body["id"] = call_id
    request = urllib.request.Request(config["url"], data=json.dumps(body).encode(), headers={**config["headers"], "content-type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=10) as response:
        raw = response.read()
    return json.loads(raw) if raw else None


def say(text):
    emit({"type": "stream_event", "event": {"type": "message_start"}})
    emit({"type": "stream_event", "event": {"type": "content_block_delta", "delta": {"type": "text_delta", "text": text}}})
    emit({"type": "assistant", "message": {"model": "claude-stand-in-1", "content": [{"type": "text", "text": text}]}})


if "NOT-SIGNED-IN" in question:
    emit({"type": "result", "subtype": "success", "is_error": True, "result": "Not logged in · Please run /login"})
    sys.exit(1)
if "TOO-OLD" in question:
    # a Claude Code from before the options the editor starts it with: its option parser stops it
    print("error: unknown option '--tools'", file=sys.stderr)
    sys.exit(1)
if "CRASH" in question:
    print("something broke inside", file=sys.stderr)
    sys.exit(3)

tools = []
if config:
    mcp("initialize", {"protocolVersion": "2025-11-25", "capabilities": {}})
    mcp("notifications/initialized", call_id=None)
    tools = ["mcp__cowriting__" + tool["name"] for tool in mcp("tools/list")["result"]["tools"]]
emit({"type": "system", "subtype": "init", "tools": tools + (["Bash"] if "OWN-TOOL" in question else []), "model": "claude-stand-in-1"})

call = lambda name, arguments: mcp("tools/call", {"name": name, "arguments": arguments})["result"]
if "HOSTILE" in question:
    say("I will just fix it for you.")
    results = [call("write_note", {"name": "../../../thesis.md", "text": "Rewritten by the model."}), call("Write", {"file_path": "/work/thesis.md", "content": "Rewritten."}), call("Bash", {"command": "rm -rf /work"})]
    say("refused: " + ", ".join(str(result["isError"]) for result in results))
elif config:
    say("Reading.")
    document = call("read_document", {})["content"][0]["text"]
    call("write_note", {"name": "glossary.md", "text": "- subjects: see participants"})
    say(f"Line 3 says: {document.splitlines()[2]}")
else:
    # no tools: the document came with the system prompt
    say("Without tools. " + ("I have the document." if "48 subjects" in system else "I have no document."))
say(f"Asked: {question.strip().splitlines()[-1]}" if "ECHO" in question else "Done.")
emit({"type": "result", "subtype": "success", "is_error": False, "result": "Done."})
