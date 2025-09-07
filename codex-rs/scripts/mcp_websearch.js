#!/usr/bin/env node
// Minimal MCP server exposing a single web search tool via stdio.
// Supports initialize, tools/list, and tools/call for `search.query`.

const readline = require('readline');

const JSONRPC = '2.0';

function write(msg) {
  process.stdout.write(JSON.stringify(msg) + '\n');
}

function initializeResult() {
  return {
    jsonrpc: JSONRPC,
    // id patched per-request
    result: {
      protocolVersion: (process.env.MCP_SCHEMA_VERSION || '2025-06-18'),
      serverInfo: {
        name: 'mcp-websearch-simple',
        version: '0.1.0',
        title: 'Web Search (Simple)'
      },
      capabilities: {
        tools: { listChanged: false }
      },
      instructions: undefined
    }
  };
}

function makeToolsList() {
  const hasSerp = !!process.env.SERPAPI_KEY;
  const hasGoogle = !!process.env.GOOGLE_API_KEY && !!process.env.GOOGLE_CSE_ID;
  const engines = [hasGoogle ? 'google_cse' : null, hasSerp ? 'serpapi' : null].filter(Boolean);
  const desc = engines.length
    ? `Web search using: ${engines.join(', ')}.`
    : 'Web search (no API keys detected). Set GOOGLE_API_KEY+GOOGLE_CSE_ID or SERPAPI_KEY.';

  return {
    jsonrpc: JSONRPC,
    // id patched per-request
    result: {
      tools: [
        {
          name: 'search.query',
          description: desc,
          annotations: {
            openWorldHint: true,
            title: 'Search the Web'
          },
          inputSchema: {
            type: 'object',
            properties: {
              q: { type: 'string', description: 'Query string (alias: query)' },
              query: { type: 'string', description: 'Query string (alias of q)' },
              num: { type: 'number', description: 'Max results (default 5, Google allows 1-10)' },
              max_results: { type: 'number', description: 'Alias of `num` (1-10 for Google)' },
              site: { type: 'string', description: 'Optional site: filter (e.g., ai.google)' },
              engine: { type: 'string', description: 'serpapi|google_cse|google' },
              dateRestrict: { type: 'string', description: 'Google dateRestrict (e.g., d7, m1, y1)' }
            },
            // Accept either q or query; validate at runtime
            required: [],
            additionalProperties: false
          }
        }
      ]
    }
  };
}

const rl = readline.createInterface({ input: process.stdin });
rl.on('line', line => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  const { id, method, params } = msg;
  if (!method) return;
  if (method === 'initialize') {
    const res = initializeResult();
    res.id = id; write(res);
    // Send notifications/initialized (optional)
    write({ jsonrpc: JSONRPC, method: 'notifications/initialized', params: null });
    return;
  }
  if (method === 'tools/list') {
    const res = makeToolsList();
    res.id = id; write(res);
    return;
  }
  if (method === 'tools/call' && params && params.name === 'search.query') {
    (async () => {
      const { q, query: queryArg, num, max_results, site, engine, dateRestrict } = (params.arguments) || {};
      const qText = (q ?? queryArg ?? '').toString();
      if (!qText.trim()) {
        write({ jsonrpc: JSONRPC, id, result: { content: [{ type: 'text', text: 'Search error: missing query (provide `q` or `query`)' }], isError: true } });
        return;
      }
      // Google CSE only allows 1..10 for `num`.
      const nRaw = Number(num ?? max_results);
      const n = Number.isFinite(nRaw) ? Math.min(10, Math.max(1, nRaw)) : 5;
      const hasSerp = !!process.env.SERPAPI_KEY;
      const hasGoogle = !!process.env.GOOGLE_API_KEY && !!process.env.GOOGLE_CSE_ID;
      let eng = (engine || '').toLowerCase();
      if (eng === 'google') eng = 'google_cse';
      if (!eng) eng = hasGoogle ? 'google_cse' : (hasSerp ? 'serpapi' : null);
      if (!eng) {
        write({ jsonrpc: JSONRPC, id, result: { content: [{ type: 'text', text: 'No search engine configured. Provide SERPAPI_KEY or GOOGLE_API_KEY+GOOGLE_CSE_ID.' }], isError: true } });
        return;
      }

      try {
        const doSerpapi = async () => {
          const base = 'https://serpapi.com/search.json';
          const params = new URLSearchParams({ engine: 'google', q: qText || '', api_key: process.env.SERPAPI_KEY, num: String(n) });
          if (site) params.set('q', `${qText} site:${site}`);
          const url = `${base}?${params}`;
          const res = await fetch(url);
          let json; try { json = await res.json(); } catch { json = null; }
          if (!res.ok) {
            const errText = (json && json.error) || res.statusText || String(res.status);
            throw new Error(`SerpAPI HTTP ${res.status}: ${errText}`);
          }
          if (json && json.error) throw new Error(`SerpAPI error: ${json.error}`);
          return (json && json.organic_results || []).map(r => ({ title: r.title, link: r.link, snippet: r.snippet || '' }));
        };

        const doGoogle = async () => {
          const base = 'https://www.googleapis.com/customsearch/v1';
          const params = new URLSearchParams({ key: process.env.GOOGLE_API_KEY, cx: process.env.GOOGLE_CSE_ID, q: qText || '', num: String(n) });
          if (site) params.set('q', `${qText} site:${site}`);
          if (dateRestrict) params.set('dateRestrict', dateRestrict);
          const url = `${base}?${params}`;
          const res = await fetch(url);
          let json; try { json = await res.json(); } catch { json = null; }
          if (!res.ok) {
            const errText = (json && json.error && json.error.message) || res.statusText || String(res.status);
            const reasons = (json && json.error && Array.isArray(json.error.errors)) ? json.error.errors.map(e => e.reason).filter(Boolean).join(', ') : '';
            const extra = reasons ? ` (reason: ${reasons})` : '';
            throw new Error(`Google CSE HTTP ${res.status}: ${errText}${extra}`);
          }
          if (json && json.error) throw new Error(`Google CSE error: ${(json.error && json.error.message) || 'unknown'}`);
          return (json && json.items || []).map(r => ({ title: r.title, link: r.link, snippet: (r.snippet || '') }));
        };

        let items = [];
        if (eng === 'serpapi') {
          items = hasSerp ? await doSerpapi() : [];
          if (!items.length && hasGoogle) items = await doGoogle();
        } else {
          items = hasGoogle ? await doGoogle() : [];
          if (!items.length && hasSerp) items = await doSerpapi();
        }

        if (!items.length) {
          write({ jsonrpc: JSONRPC, id, result: { content: [{ type: 'text', text: 'No results.' }] } });
          return;
        }
        const lines = items.slice(0, n).map((it, i) => `${i + 1}. ${it.title}\n   ${it.link}\n   ${it.snippet}`);
        write({ jsonrpc: JSONRPC, id, result: { content: [{ type: 'text', text: lines.join('\n\n') }] } });
      } catch (e) {
        const msg = (e && e.message) || String(e);
        write({ jsonrpc: JSONRPC, id, result: { content: [{ type: 'text', text: `Search error: ${msg}` }], isError: true } });
      }
    })();
    return;
  }
  // Not implemented: only send errors for requests (id present); ignore notifications.
  if (id !== undefined) {
    write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } });
  }
});
