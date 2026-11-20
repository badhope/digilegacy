#!/usr/bin/env python3
"""Minimal OpenAI-compatible sidecar for a local GGUF model.

Used by the DigiLegacy local-model manager. It intentionally exposes only the
surface the planner needs: /health, /v1/models and /v1/chat/completions.

Requirements: python3, fastapi, uvicorn, jinja2, llama-cpp-python.
"""

from __future__ import annotations

import argparse
import json
import threading
import time
from pathlib import Path

import jinja2
import uvicorn
from fastapi import FastAPI, Request
from llama_cpp import Llama, LlamaGrammar

app = FastAPI()
llm: Llama | None = None
chat_template: jinja2.Template | None = None
model_id = ""
lock = threading.Lock()

PLAN_SCHEMA = {
    "type": "object",
    "properties": {
        "steps": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "tool": {"type": "string"},
                    "args": {"type": "object"},
                },
                "required": ["tool", "args"],
                "additionalProperties": False,
            },
        }
    },
    "required": ["steps"],
    "additionalProperties": False,
}
PLAN_GRAMMAR: LlamaGrammar | None = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-path", required=True)
    parser.add_argument("--model-id", default="")
    parser.add_argument("--port", type=int, default=11435)
    parser.add_argument("--ctx", type=int, default=8192)
    parser.add_argument("--threads", type=int, default=2)
    return parser.parse_args()


def render(messages: list[dict], enable_thinking: bool = False) -> str:
    assert chat_template is not None
    values = {
        "bos_token": "<s>",
        "messages": messages,
        "add_generation_prompt": True,
        "enable_thinking": enable_thinking,
    }
    try:
        return chat_template.render(**values)
    except jinja2.TemplateError:
        values.pop("enable_thinking", None)
        return chat_template.render(**values)


@app.get("/health")
def health():
    return {"status": "ok", "model": model_id}


@app.get("/v1/models")
def models():
    return {"object": "list", "data": [{"id": model_id, "object": "model", "owned_by": "local"}]}


@app.post("/v1/chat/completions")
async def completions(request: Request):
    global llm, PLAN_GRAMMAR
    body = await request.json()
    messages = body.get("messages", [])
    max_tokens = int(body.get("max_tokens") or 1024)
    temperature = float(body.get("temperature", 0.7))
    enable_thinking = bool(body.get("enable_thinking", False))
    response_format = body.get("response_format") or {}
    wants_json = response_format.get("type") == "json_object"
    if not wants_json:
        wants_json = any(
            isinstance(message.get("content"), str) and "只输出 JSON 对象" in message["content"]
            for message in messages
        )
    prompt = render(messages, enable_thinking=enable_thinking)
    if wants_json and PLAN_GRAMMAR is None:
        PLAN_GRAMMAR = LlamaGrammar.from_json_schema(json.dumps(PLAN_SCHEMA))
    started = time.perf_counter()
    with lock:
        out = llm.create_completion(
            prompt=prompt,
            max_tokens=min(max_tokens, 2048),
            temperature=temperature,
            top_p=0.95,
            min_p=0.0,
            stop=["<|im_end|>"],
            echo=False,
            grammar=PLAN_GRAMMAR if wants_json else None,
        )
    content = out["choices"][0]["text"]
    usage = out.get("usage", {})
    return {
        "id": "chatcmpl-local",
        "object": "chat.completion",
        "created": int(time.time()),
        "model": model_id,
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": content},
                "finish_reason": "stop",
            }
        ],
        "usage": {
            "prompt_tokens": usage.get("prompt_tokens", 0),
            "completion_tokens": usage.get("completion_tokens", 0),
            "total_tokens": usage.get("total_tokens", 0),
        },
        "local_elapsed_ms": int((time.perf_counter() - started) * 1000),
    }


def main() -> int:
    global llm, chat_template, model_id
    args = parse_args()
    model_id = args.model_id or Path(args.model_path).stem
    llm = Llama(
        model_path=args.model_path,
        n_ctx=args.ctx,
        n_batch=256,
        n_threads=args.threads,
        verbose=False,
    )
    template = llm.metadata.get("tokenizer.chat_template")
    if not template:
        raise SystemExit("GGUF 缺少 chat template，无法作为对话模型运行")
    chat_template = jinja2.Template(template)
    uvicorn.run(app, host="127.0.0.1", port=args.port, log_level="warning")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
