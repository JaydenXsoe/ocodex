#!/usr/bin/env node
// Minimal MCP server for code indexing: search and read files within CWD.
// Implements initialize, tools/list, and tools/call over stdio line-delimited JSON.

const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const rl = require('readline').createInterface({ input: process.stdin });

const JSONRPC = '2.0';

function write(msg) { process.stdout.write(JSON.stringify(msg) + '\n'); }

function inProject(p) {
  try {
    const abs = path.resolve(p);
    const cwd = process.cwd();
    const rel = path.relative(cwd, abs);
    return !!rel && !rel.startsWith('..') && !path.isAbsolute(rel);
  } catch { return false; }
}

function initializeResult(id) {
  return {
    jsonrpc: JSONRPC,
    id,
    result: {
      protocolVersion: process.env.MCP_SCHEMA_VERSION || '2025-06-18',
      serverInfo: { name: 'mcp-codeindex', version: '0.1.0', title: 'Code Index' },
      capabilities: { tools: { listChanged: false } },
      instructions: 'Use code.search to locate snippets, then code.read to fetch exact ranges.'
    }
  };
}

function toolsList(id) {
  return {
    jsonrpc: JSONRPC,
    id,
    result: {
      tools: [
        {
          name: 'code.search',
          description: 'Search repository files using ripgrep if available (fallback to grep).',
          annotations: { title: 'Code Search' },
          inputSchema: {
            type: 'object',
            properties: {
              q: { type: 'string', description: 'Query string (regex supported by rg/grep)' },
              globs: { type: 'array', items: { type: 'string' }, description: 'Optional include globs (e.g., src/**/*.rs)' },
              max_results: { type: 'number', description: 'Maximum matches to return (default 50)' },
              context: { type: 'number', description: 'Lines of context before/after (default 1)' }
            },
            required: ['q'],
            additionalProperties: false
          }
        },
        {
          name: 'code.read',
          description: 'Read a file or a line range within the current project.',
          annotations: { title: 'Read File' },
          inputSchema: {
            type: 'object',
            properties: {
              path: { type: 'string', description: 'Relative file path' },
              start: { type: 'number', description: '1-based start line (optional)' },
              end: { type: 'number', description: '1-based end line inclusive (optional)' },
              max_bytes: { type: 'number', description: 'Maximum bytes to return (default 64KiB)' }
            },
            required: ['path'],
            additionalProperties: false
          }
        }
      ]
    }
  };
}

function callCodeSearch(params) {
  const q = String(params.q || '').trim();
  if (!q) return { error: 'empty query' };
  const max = Math.max(1, Math.min(Number(params.max_results || 50), 500));
  const ctx = Math.max(0, Math.min(Number(params.context || 1), 10));
  const globs = Array.isArray(params.globs) ? params.globs : [];
  const hasRg = (() => { try { cp.execSync('rg --version', { stdio: 'ignore' }); return true; } catch { return false; } })();
  if (hasRg) {
    const args = ['--color', 'never', '--line-number', '--column', `-C${ctx}`, '-n', '-S'];
    globs.forEach(g => { args.push('-g'); args.push(g); });
    args.push(q);
    args.push('.');
    let out = '';
    try { out = cp.execFileSync('rg', args, { encoding: 'utf8', maxBuffer: 1024 * 1024 }); }
    catch (e) { out = e.stdout?.toString?.() || ''; }
    const lines = out.split(/\r?\n/).filter(Boolean);
    const results = [];
    for (const line of lines) {
      // Format: file:line:col:content  (context lines begin with '-')
      if (line.startsWith('-') || line.startsWith('+')) continue;
      const m = line.match(/^(.*?):(\d+):(\d+):(.*)$/);
      if (!m) continue;
      const file = m[1];
      const lno = Number(m[2]);
      const col = Number(m[3]);
      const preview = m[4];
      results.push({ file, line: lno, column: col, preview });
      if (results.length >= max) break;
    }
    return { results };
  } else {
    // Fallback: grep -R
    const args = ['-R', '-n', '-H'];
    globs.forEach(g => { args.push('--include'); args.push(g); });
    args.push(q); args.push('.');
    let out = '';
    try { out = cp.execFileSync('grep', args, { encoding: 'utf8', maxBuffer: 1024 * 1024 }); }
    catch (e) { out = e.stdout?.toString?.() || ''; }
    const lines = out.split(/\r?\n/).filter(Boolean);
    const results = [];
    for (const line of lines) {
      const m = line.match(/^(.*?):(\d+):(.*)$/);
      if (!m) continue;
      const file = m[1];
      const lno = Number(m[2]);
      const preview = m[3];
      results.push({ file, line: lno, column: 1, preview });
      if (results.length >= max) break;
    }
    return { results };
  }
}

function callCodeRead(params) {
  const p = String(params.path || '');
  if (!p || !inProject(p)) return { error: 'invalid path' };
  const abs = path.resolve(p);
  if (!fs.existsSync(abs) || !fs.statSync(abs).isFile()) return { error: 'not a file' };
  const maxBytes = Math.max(1024, Math.min(Number(params.max_bytes || 64 * 1024), 2 * 1024 * 1024));
  const data = fs.readFileSync(abs, 'utf8');
  if (params.start || params.end) {
    const start = Math.max(1, Number(params.start || 1));
    const end = Math.max(start, Number(params.end || start + 200 - 1));
    const lines = data.split(/\r?\n/);
    const slice = lines.slice(start - 1, end).join('\n');
    const content = slice.slice(0, maxBytes);
    return { content, start, end, truncated: content.length < slice.length };
  }
  const content = data.slice(0, maxBytes);
  return { content, truncated: content.length < data.length };
}

function toolsCall(id, method, params) {
  try {
    if (method === 'code.search') {
      return { jsonrpc: JSONRPC, id, result: callCodeSearch(params || {}) };
    }
    if (method === 'code.read') {
      return { jsonrpc: JSONRPC, id, result: callCodeRead(params || {}) };
    }
    return { jsonrpc: JSONRPC, id, error: { code: -32601, message: `Unknown tool: ${method}` } };
  } catch (e) {
    return { jsonrpc: JSONRPC, id, error: { code: -32000, message: String(e && e.message || e) } };
  }
}

rl.on('line', line => {
  let msg; try { msg = JSON.parse(line); } catch { return; }
  const { id, method, params } = msg || {};
  if (!method) return;
  if (method === 'initialize') { write(initializeResult(id)); return; }
  if (method === 'tools/list') { write(toolsList(id)); return; }
  if (method === 'tools/call') { write(toolsCall(id, params?.name, params?.arguments)); return; }
  if (id !== undefined) { write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } }); }
});

