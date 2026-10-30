"""Minimal stdio MCP server used to verify CodeLoom's MCP token accounting."""

import json
import sys


def respond(request):
    method = request.get("method")
    request_id = request.get("id")
    params = request.get("params") or {}

    if method == "notifications/initialized":
        return None
    if method == "initialize":
        result = {
            "protocolVersion": params.get("protocolVersion", "2025-03-26"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "codeloom-token-demo", "version": "1.0.0"},
        }
    elif method == "ping":
        result = {}
    elif method == "tools/list":
        result = {
            "tools": [
                {
                    "name": "token_demo_echo",
                    "description": "Echo text locally to verify that the MCP demo tool works.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": {"type": "string", "description": "Text to echo."}
                        },
                        "required": ["text"],
                    },
                }
            ]
        }
    elif method == "tools/call" and params.get("name") == "token_demo_echo":
        text = (params.get("arguments") or {}).get("text", "")
        result = {"content": [{"type": "text", "text": str(text)}], "isError": False}
    else:
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": -32601, "message": "Method not found"},
        }

    return {"jsonrpc": "2.0", "id": request_id, "result": result}


for line in sys.stdin:
    try:
        response = respond(json.loads(line))
        if response is not None:
            sys.stdout.write(json.dumps(response, separators=(",", ":")) + "\n")
            sys.stdout.flush()
    except (json.JSONDecodeError, TypeError, ValueError):
        continue
