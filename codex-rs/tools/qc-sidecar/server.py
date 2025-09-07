#!/usr/bin/env python3
"""
Canonical QC sidecar simulator for ocodex.
This is the only copy; ocodex-labs should point to this tool (or a packaged image).

Endpoint:
  POST /optimize  Content-Type: application/json
  Body: QuboInstance (see ocodex-labs/docs/CORE/qc_schemas/qubo_instance.schema.json)
  Returns: ScheduleDelta JSON

Algorithm:
  - Build a precedence graph and perform a priority-aware topological sort.
  - Run a tiny annealing pass to reduce write-lock conflicts per capacity bucket.
"""
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import random
from typing import Dict, List, Any


def topo_sort_with_priority(tasks: List[Dict[str, Any]]) -> List[str]:
    deps = {t["id"]: set(t.get("depends_on", []) or []) for t in tasks}
    prio = {t["id"]: int(t.get("priority", 0)) for t in tasks}
    ids = set(deps.keys())
    ready = [tid for tid, ds in deps.items() if not ds]
    order: List[str] = []
    while ready:
        ready.sort(key=lambda i: prio.get(i, 0), reverse=True)
        n = ready.pop(0)
        order.append(n)
        for k in list(ids):
            if n in deps.get(k, set()):
                deps[k].discard(n)
                if not deps[k] and k not in order and k not in ready:
                    ready.append(k)
        ids.discard(n)
        deps.pop(n, None)
    order.extend([i for i in deps.keys() if i not in order])
    return order


def anneal_order(order: List[str], write_flags: Dict[str, bool], capacity: int, steps: int = 200) -> List[str]:
    def cost(seq: List[str]) -> int:
        write_cap = 1
        violations = 0
        for i in range(0, len(seq), capacity):
            bucket = seq[i : i + capacity]
            writes = sum(1 for tid in bucket if write_flags.get(tid, False))
            violations += max(0, writes - write_cap)
        return violations

    best = list(order)
    best_c = cost(best)
    cur = list(order)
    cur_c = best_c
    rng = random.Random(42)
    for _ in range(steps):
        i, j = rng.randrange(0, len(cur)), rng.randrange(0, len(cur))
        if i == j:
            continue
        cur[i], cur[j] = cur[j], cur[i]
        c = cost(cur)
        if c <= cur_c or rng.random() < 0.05:
            cur_c = c
            if c < best_c:
                best = list(cur)
                best_c = c
        else:
            cur[i], cur[j] = cur[j], cur[i]
    return best


class Handler(BaseHTTPRequestHandler):
    def _set_headers(self, code=200):
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.end_headers()

    def do_POST(self):  # noqa: N802
        if self.path != "/optimize":
            self._set_headers(404)
            self.wfile.write(b"{}")
            return
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        try:
            inst = json.loads(body.decode("utf-8"))
        except Exception:
            self._set_headers(400)
            self.wfile.write(b"{}")
            return
        tasks = inst.get("tasks", [])
        horizon = inst.get("horizon", {})
        capacity = int(horizon.get("capacity", 1))
        order = topo_sort_with_priority(tasks)
        write_flags = {t["id"]: bool(t.get("write", False)) for t in tasks}
        order2 = anneal_order(order, write_flags, capacity)
        delta = {
            "order": order2,
            "priority_bumps": [],
            "deferrals": [],
            "cancellations": [],
            "confidence": 0.6 if order2 != order else 0.5,
            "metadata": {"initial_order": order, "improved": order2 != order},
        }
        self._set_headers(200)
        self.wfile.write(json.dumps(delta).encode("utf-8"))


def run(host="127.0.0.1", port=5057):
    httpd = HTTPServer((host, port), Handler)
    print(f"qc-sidecar listening on http://{host}:{port}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        httpd.server_close()


if __name__ == "__main__":
    run()

