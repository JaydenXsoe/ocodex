#!/usr/bin/env node
// Minimal MCP server exposing a single web search tool via stdio.
// Supports initialize and tools/list. tools/call is optional here.

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
              q: { type: 'string', description: 'Query string' },
              num: { type: 'number', description: 'Max results (default 5)' },
              site: { type: 'string', description: 'Optional site: filter (e.g., ai.google)' },
              engine: { type: 'string', description: 'serpapi|google_cse' },
              dateRestrict: { type: 'string', description: 'Google dateRestrict (e.g., d7, m1, y1)' }
            },
            required: ['q'],
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
  const { id, method } = msg;
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
  // Not implemented: only send errors for requests (id present); ignore notifications.
  if (id !== undefined) {
    write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } });
  }
});
