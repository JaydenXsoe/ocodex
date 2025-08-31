#!/usr/bin/env node
// Minimal MCP server for git inspection: status and diff.

const cp = require('child_process');
const rl = require('readline').createInterface({ input: process.stdin });
const JSONRPC = '2.0';
function write(msg) { process.stdout.write(JSON.stringify(msg) + '\n'); }
function run(cmd, args) {
  try {
    const out = cp.spawnSync(cmd, args, { encoding: 'utf8', maxBuffer: 8*1024*1024 });
    return { status: out.status ?? 1, stdout: out.stdout || '', stderr: out.stderr || '' };
  } catch (e) { return { status: 127, stdout: '', stderr: String(e && e.message || e) }; }
}
function initialize(id) { return { jsonrpc: JSONRPC, id, result: { protocolVersion: process.env.MCP_SCHEMA_VERSION || '2025-06-18', serverInfo: { name: 'mcp-git', version: '0.1.0', title: 'Git Tools' }, capabilities: { tools: { listChanged: false } } } }; }
function listTools(id) { return { jsonrpc: JSONRPC, id, result: { tools: [
  { name: 'git.status', description: 'Show repository status', inputSchema: { type: 'object', properties: {}, additionalProperties: false } },
  { name: 'git.diff', description: 'Show diff (staged+unstaged)', inputSchema: { type: 'object', properties: { staged: { type: 'boolean' } }, additionalProperties: false } },
] } }; }
function call(id, name, params) {
  try {
    if (name === 'git.status') return { jsonrpc: JSONRPC, id, result: run('git', ['status', '--porcelain=v1']) };
    if (name === 'git.diff') {
      const staged = !!(params && params.staged);
      return { jsonrpc: JSONRPC, id, result: staged ? run('git', ['diff', '--staged']) : run('git', ['diff']) };
    }
    return { jsonrpc: JSONRPC, id, error: { code: -32601, message: `Unknown tool: ${name}` } };
  } catch (e) { return { jsonrpc: JSONRPC, id, error: { code: -32000, message: String(e && e.message || e) } }; }
}
rl.on('line', line => { let msg; try { msg = JSON.parse(line); } catch { return; } const { id, method, params } = msg || {}; if (method === 'initialize') { write(initialize(id)); return; } if (method === 'tools/list') { write(listTools(id)); return; } if (method === 'tools/call') { write(call(id, params?.name, params?.arguments)); return; } if (id !== undefined) { write({ jsonrpc: JSONRPC, id, error: { code: -32601, message: `Method not implemented: ${method}` } }); } });

